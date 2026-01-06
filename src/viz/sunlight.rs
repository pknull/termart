//! Sunlight - Day/night cycle visualization with screen temperature control
//!
//! Shows a sine wave representing the day cycle:
//! - Peak (top) = solar noon, cool blue colors
//! - Trough (bottom) = midnight, warm amber/red colors
//! - Current time shown as a moving dot
//! - Sunrise/sunset marked on the wave
//!
//! Optionally adjusts screen color temperature via xrandr gamma.

use crate::terminal::Terminal;
use chrono::{Local, Timelike};
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::io;
use std::process::Command;
use sunrise_sunset_calculator::SunriseSunsetParameters;

pub struct SunlightConfig {
    pub time_step: f32,
    pub latitude: f64,
    pub longitude: f64,
    pub adjust_gamma: bool,
    pub demo: bool,           // Cycle through day quickly instead of real time
    pub demo_speed: f32,      // Hours per second in demo mode
    pub night_blue: f64,      // Minimum blue gamma at night (0.0-1.0)
    pub night_green: f64,     // Minimum green gamma at night (0.0-1.0)
}

impl Default for SunlightConfig {
    fn default() -> Self {
        // Use actual 3400K values from kelvin_to_gamma
        let (_, g, b) = kelvin_to_gamma(3400);
        Self {
            time_step: 0.1,
            latitude: 40.7128,   // NYC default
            longitude: -74.0060,
            adjust_gamma: true,
            demo: false,
            demo_speed: 2.0,     // 2 hours per second = full day in 12 seconds
            night_green: g,      // 3400K (f.lux default)
            night_blue: b,       // 3400K (f.lux default)
        }
    }
}

/// Convert color temperature in Kelvin to RGB gamma values
/// Based on Tanner Helland's algorithm (commonly used by redshift/f.lux)
pub fn kelvin_to_gamma(kelvin: u32) -> (f64, f64, f64) {
    let temp = (kelvin as f64 / 100.0).clamp(10.0, 400.0);

    // Minimum gamma value - xrandr requires positive values
    let min_gamma = 0.1;

    // Red
    let r = if temp <= 66.0 {
        1.0
    } else {
        let r = 329.698727446 * (temp - 60.0).powf(-0.1332047592);
        (r / 255.0).clamp(min_gamma, 1.0)
    };

    // Green
    let g = if temp <= 66.0 {
        let g = 99.4708025861 * temp.ln() - 161.1195681661;
        (g / 255.0).clamp(min_gamma, 1.0)
    } else {
        let g = 288.1221695283 * (temp - 60.0).powf(-0.0755148492);
        (g / 255.0).clamp(min_gamma, 1.0)
    };

    // Blue
    let b = if temp >= 66.0 {
        1.0
    } else if temp <= 19.0 {
        min_gamma
    } else {
        let b = 138.5177312231 * (temp - 10.0).ln() - 305.0447927307;
        (b / 255.0).clamp(min_gamma, 1.0)
    };

    (r, g, b)
}

struct SolarTimes {
    sunrise_hour: f64,  // Hours since midnight (e.g., 6.5 = 6:30 AM)
    sunset_hour: f64,   // Hours since midnight (e.g., 18.75 = 6:45 PM)
}

fn calculate_solar_times(lat: f64, lon: f64, unix_time: i64) -> SolarTimes {
    let params = SunriseSunsetParameters::new(unix_time, lat, lon);
    let result = params.calculate();

    // Convert Unix timestamp to hour of day (local time)
    let unix_to_local_hour = |ts: i64| -> f64 {
        let dt = chrono::DateTime::from_timestamp(ts, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let local = dt.with_timezone(&chrono::Local);
        local.hour() as f64 + local.minute() as f64 / 60.0 + local.second() as f64 / 3600.0
    };

    match result {
        Ok(r) => {
            let sunrise_hour = unix_to_local_hour(r.rise);
            let sunset_hour = unix_to_local_hour(r.set);
            SolarTimes {
                sunrise_hour,
                sunset_hour,
            }
        }
        Err(_) => {
            // Fallback to approximate times
            SolarTimes {
                sunrise_hour: 6.0,
                sunset_hour: 18.0,
            }
        }
    }
}

/// f.lux-style phases
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Night,    // Full warm - midnight to pre-dawn
    Sunrise,  // Transition warm->cool - pre-dawn to post-sunrise
    Day,      // Full cool - post-sunrise to pre-sunset
    Sunset,   // Transition cool->warm - pre-sunset to post-sunset
}

impl Phase {
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Night => "Night",
            Phase::Sunrise => "Sunrise",
            Phase::Day => "Day",
            Phase::Sunset => "Sunset",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Phase::Night => Color::Red,
            Phase::Sunrise => Color::Yellow,
            Phase::Day => Color::Cyan,
            Phase::Sunset => Color::DarkYellow,
        }
    }
}

/// Determine current phase and temperature factor
/// Returns (phase, temperature) where temperature is 0.0 (warm/night) to 1.0 (cool/day)
fn calculate_phase_and_temp(hour: f64, solar: &SolarTimes) -> (Phase, f64) {
    let transition_duration = 1.0; // 1 hour transitions (like f.lux)

    let sunrise_start = solar.sunrise_hour - transition_duration / 2.0;
    let sunrise_end = solar.sunrise_hour + transition_duration / 2.0;
    let sunset_start = solar.sunset_hour - transition_duration / 2.0;
    let sunset_end = solar.sunset_hour + transition_duration / 2.0;

    if hour < sunrise_start || hour >= sunset_end {
        // Night - full warm
        (Phase::Night, 0.0)
    } else if hour < sunrise_end {
        // Sunrise transition - warm to cool
        let progress = (hour - sunrise_start) / transition_duration;
        let temp = ease_in_out(progress);
        (Phase::Sunrise, temp)
    } else if hour < sunset_start {
        // Day - full cool
        (Phase::Day, 1.0)
    } else {
        // Sunset transition - cool to warm
        let progress = (hour - sunset_start) / transition_duration;
        let temp = 1.0 - ease_in_out(progress);
        (Phase::Sunset, temp)
    }
}

/// Smooth easing function for natural transitions
fn ease_in_out(t: f64) -> f64 {
    // Smoothstep: 3t² - 2t³
    t * t * (3.0 - 2.0 * t)
}

/// Convert temperature factor to RGB gamma values for xrandr
fn temp_to_gamma(temp: f64, night_green: f64, night_blue: f64) -> (f64, f64, f64) {
    // temp: 0.0 = warm (night), 1.0 = cool (day)
    // Warm: boost red, reduce blue (like 2700K)
    // Cool: neutral (like 6500K)

    // xrandr requires positive gamma values (not 0)
    let min_gamma = 0.1;

    let r = 1.0;
    let g = (night_green + temp * (1.0 - night_green)).max(min_gamma);
    let b = (night_blue + temp * (1.0 - night_blue)).max(min_gamma);

    (r, g, b)
}

/// Apply gamma via xrandr
fn apply_gamma(r: f64, g: f64, b: f64) -> io::Result<()> {
    // Get list of connected outputs
    let output = Command::new("xrandr")
        .arg("--query")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find connected outputs and apply gamma to each
    for line in stdout.lines() {
        if line.contains(" connected") {
            if let Some(output_name) = line.split_whitespace().next() {
                let _ = Command::new("xrandr")
                    .args(["--output", output_name, "--gamma", &format!("{:.2}:{:.2}:{:.2}", r, g, b)])
                    .output();
            }
        }
    }

    Ok(())
}

/// Reset gamma to normal (1:1:1)
fn reset_gamma() -> io::Result<()> {
    apply_gamma(1.0, 1.0, 1.0)
}

/// Pick color based on temperature (0=night/warm, 1=day/cool)
fn temp_color(t: f64) -> Color {
    match t {
        x if x < 0.25 => Color::Red,
        x if x < 0.5 => Color::DarkYellow,
        x if x < 0.75 => Color::Cyan,
        _ => Color::Blue,
    }
}

pub fn run(config: SunlightConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;

    // Color palette (16-color)
    let sunrise_color = Color::Yellow;
    let sunset_color = Color::DarkYellow;
    let dot_color = Color::White;
    let text_color = Color::DarkGrey;

    let (mut w, mut h) = term.size();
    // In demo mode, update gamma every frame; otherwise every minute
    let gamma_update_interval = if config.demo {
        std::time::Duration::from_millis(50)
    } else {
        std::time::Duration::from_secs(60)
    };
    // Set to past so first update happens immediately
    let mut last_gamma_update = std::time::Instant::now() - gamma_update_interval;

    // Track if we've applied gamma (for cleanup)
    let mut gamma_applied = false;

    // Demo mode: simulated hour - start at current time
    let now_init = Local::now();
    let mut demo_hour: f64 = now_init.hour() as f64 + now_init.minute() as f64 / 60.0;

    // Apply gamma immediately on startup (don't wait for interval)
    if config.adjust_gamma {
        let solar = calculate_solar_times(config.latitude, config.longitude, now_init.timestamp());
        let init_hour = if config.demo { demo_hour } else {
            now_init.hour() as f64 + now_init.minute() as f64 / 60.0 + now_init.second() as f64 / 3600.0
        };
        let (_, temp) = calculate_phase_and_temp(init_hour, &solar);
        let (r, g, b) = temp_to_gamma(temp, config.night_green, config.night_blue);
        let _ = apply_gamma(r, g, b);
        gamma_applied = true;
    }

    loop {
        // Handle input
        if let Ok(Some((KeyCode::Char('q') | KeyCode::Esc, _))) = term.check_key() {
            break;
        }

        // Handle resize
        if let Ok((new_w, new_h)) = size() {
            if new_w != w || new_h != h {
                w = new_w;
                h = new_h;
                term.resize(w, h);
                term.clear_screen()?;
            }
        }

        let now = Local::now();

        // In demo mode, use simulated time; otherwise use real time
        let current_hour = if config.demo {
            demo_hour += config.demo_speed as f64 * config.time_step as f64;
            if demo_hour >= 24.0 {
                demo_hour -= 24.0;
            }
            demo_hour
        } else {
            now.hour() as f64 + now.minute() as f64 / 60.0 + now.second() as f64 / 3600.0
        };

        // Calculate solar times for today
        let solar = calculate_solar_times(config.latitude, config.longitude, now.timestamp());

        // Calculate phase and temperature (f.lux style)
        let (phase, temp) = calculate_phase_and_temp(current_hour, &solar);

        // Apply gamma if enabled and interval passed
        if config.adjust_gamma && last_gamma_update.elapsed() >= gamma_update_interval {
            let (r, g, b) = temp_to_gamma(temp, config.night_green, config.night_blue);
            let _ = apply_gamma(r, g, b);
            gamma_applied = true;
            last_gamma_update = std::time::Instant::now();
        }

        // Draw
        term.clear();

        let cx = w as usize / 2;
        let wave_width = (w as usize).saturating_sub(10);
        let wave_start_x = (w as usize).saturating_sub(wave_width) / 2;

        // Total content: wave + 4 lines below (sunrise/sunset, gamma, markers, time/loc)
        // Scale wave to fit: leave 5 lines for info, use rest for wave
        // Can go flat (0) for very small terminals
        let available = (h as usize).saturating_sub(6);
        let wave_height = available.min(10);

        // Center the whole block (wave + 4 info lines)
        let total_height = wave_height + 4;
        let block_top = (h as usize).saturating_sub(total_height) / 2;
        let wave_top_y = block_top;

        // Draw wave
        for x in 0..wave_width {
            // Map x to hours (0-24)
            let hour = x as f64 / wave_width as f64 * 24.0;

            // Calculate wave y position (sine wave, noon at top, midnight at bottom)
            let hours_from_noon = hour - 12.0;
            let normalized = hours_from_noon / 12.0;
            let wave_val = (-normalized * std::f64::consts::PI).cos(); // -1 to 1
            let y_offset = ((1.0 - wave_val) / 2.0 * wave_height as f64) as usize;
            let y = wave_top_y + y_offset;

            // Calculate color based on position on wave
            let pos_temp = (wave_val + 1.0) / 2.0; // 0 to 1
            let color = temp_color(pos_temp);

            // Draw wave point
            let screen_x = (wave_start_x + x) as i32;
            let screen_y = y as i32;

            if screen_y >= 0 && screen_y < h as i32 {
                term.set(screen_x, screen_y, '─', Some(color), false);
            }

            // Mark sunrise
            if (hour - solar.sunrise_hour).abs() < 0.5 && screen_y >= 0 && screen_y < h as i32 {
                term.set(screen_x, screen_y, '☀', Some(sunrise_color), false);
            }

            // Mark sunset
            if (hour - solar.sunset_hour).abs() < 0.5 && screen_y >= 0 && screen_y < h as i32 {
                term.set(screen_x, screen_y, '☾', Some(sunset_color), false);
            }

            // Mark current time
            if (hour - current_hour).abs() < 0.25 && screen_y >= 0 && screen_y < h as i32 {
                term.set(screen_x, screen_y, '●', Some(dot_color), false);
            }
        }

        // Draw sunrise/sunset info
        let sunrise_str = format!("↑ {:02}:{:02}",
            solar.sunrise_hour as u32,
            ((solar.sunrise_hour % 1.0) * 60.0) as u32);
        let sunset_str = format!("↓ {:02}:{:02}",
            solar.sunset_hour as u32,
            ((solar.sunset_hour % 1.0) * 60.0) as u32);

        let info_y = (wave_top_y + wave_height + 1) as i32;
        term.set_str(wave_start_x as i32, info_y, &sunrise_str, Some(sunrise_color), false);
        term.set_str((wave_start_x + wave_width - sunset_str.len()) as i32, info_y, &sunset_str, Some(sunset_color), false);

        // Draw phase and gamma info
        let (r, g, b) = temp_to_gamma(temp, config.night_green, config.night_blue);
        let phase_gamma_str = format!("{:8} γ {:.2}:{:.2}:{:.2}", phase.name(), r, g, b);
        term.set_str(cx.saturating_sub(phase_gamma_str.len() / 2) as i32, info_y + 1, &format!("{:8}", phase.name()), Some(phase.color()), false);
        term.set_str((cx.saturating_sub(phase_gamma_str.len() / 2) + 9) as i32, info_y + 1, &format!("γ {:.2}:{:.2}:{:.2}", r, g, b), Some(text_color), false);

        // Draw hour markers
        let marker_y = (wave_top_y + wave_height + 3) as i32;
        for hour in [0, 6, 12, 18, 24] {
            let x = wave_start_x + (hour as f64 / 24.0 * wave_width as f64) as usize;
            let label = format!("{:02}", hour % 24);
            term.set_str(x.saturating_sub(1) as i32, marker_y, &label, Some(text_color), false);
        }

        // Draw time and location on same line (last line of block)
        let time_loc_y = marker_y + 1;

        // In demo mode show simulated time (HH:MM:00), otherwise real time
        let (disp_hour, disp_min, disp_sec) = if config.demo {
            let h = current_hour as u32;
            let m = ((current_hour % 1.0) * 60.0) as u32;
            (h, m, 0)
        } else {
            (now.hour(), now.minute(), now.second())
        };

        let time_loc_str = format!("{:02}:{:02}:{:02}  {:.2}°{} {:.2}°{}",
            disp_hour, disp_min, disp_sec,
            config.latitude.abs(),
            if config.latitude >= 0.0 { "N" } else { "S" },
            config.longitude.abs(),
            if config.longitude >= 0.0 { "E" } else { "W" });
        let time_color = temp_color(temp);
        let time_x = cx.saturating_sub(time_loc_str.len() / 2);
        // Draw time part in temp color, location in gray
        let time_part = format!("{:02}:{:02}:{:02}", disp_hour, disp_min, disp_sec);
        term.set_str(time_x as i32, time_loc_y, &time_part, Some(time_color), false);
        let loc_str = format!("  {:.2}°{} {:.2}°{}",
            config.latitude.abs(),
            if config.latitude >= 0.0 { "N" } else { "S" },
            config.longitude.abs(),
            if config.longitude >= 0.0 { "E" } else { "W" });
        term.set_str((time_x + time_part.len()) as i32, time_loc_y, &loc_str, Some(text_color), false);

        term.present()?;
        term.sleep(config.time_step);
    }

    // Reset gamma on exit
    if gamma_applied {
        let _ = reset_gamma();
    }

    Ok(())
}
