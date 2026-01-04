//! Clock widget - 24-hour time in block letters with date

use crate::colors::ColorState;
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use chrono::{Datelike, Local, Timelike};
use std::env;
use std::fs;
use std::io;

/// Get timezone abbreviation from system (cached)
fn get_tz_abbrev() -> &'static str {
    use std::sync::OnceLock;
    static TZ_ABBREV: OnceLock<String> = OnceLock::new();
    TZ_ABBREV.get_or_init(|| {
        // Try TZ environment variable first
        if let Ok(tz) = env::var("TZ") {
            let tz_name = if tz.starts_with(':') { &tz[1..] } else { &tz };
            if !tz_name.starts_with('/') {
                return tz_to_abbrev(tz_name);
            }
        }
        // Try /etc/timezone (Debian/Ubuntu)
        if let Ok(tz) = fs::read_to_string("/etc/timezone") {
            return tz_to_abbrev(tz.trim());
        }
        // Try /etc/localtime symlink target (other Linux)
        if let Ok(link) = fs::read_link("/etc/localtime") {
            if let Some(tz) = link.to_str() {
                if let Some(pos) = tz.find("zoneinfo/") {
                    return tz_to_abbrev(&tz[pos + 9..]);
                }
            }
        }
        // Fallback to offset
        let offset = Local::now().format("%:z").to_string();
        format!("UTC{}", offset.replace(":00", "").replace(":30", ".5"))
    })
}

/// Convert IANA timezone to common abbreviation
#[inline]
fn tz_to_abbrev(tz: &str) -> String {
    match tz {
        "America/New_York" | "US/Eastern" => "EST",
        "America/Chicago" | "US/Central" => "CST",
        "America/Denver" | "US/Mountain" | "America/Phoenix" => "MST",
        "America/Los_Angeles" | "US/Pacific" => "PST",
        "America/Anchorage" | "US/Alaska" => "AKST",
        "Pacific/Honolulu" | "US/Hawaii" => "HST",
        "Europe/London" | "GB" => "GMT",
        "Europe/Paris" | "Europe/Berlin" | "Europe/Amsterdam" => "CET",
        "Europe/Moscow" => "MSK",
        "Asia/Tokyo" | "Japan" => "JST",
        "Asia/Shanghai" | "Asia/Hong_Kong" => "HKT",
        "Asia/Kolkata" | "Asia/Calcutta" => "IST",
        "Asia/Dubai" => "GST",
        "Australia/Sydney" => "AEST",
        "Australia/Perth" => "AWST",
        "Pacific/Auckland" | "NZ" => "NZST",
        "UTC" | "Etc/UTC" => "UTC",
        _ => {
            // Fallback: extract uppercase letters
            let abbrev: String = tz.split('/').last().unwrap_or(tz)
                .chars()
                .filter(|c| c.is_uppercase())
                .take(3)
                .collect();
            return abbrev;
        }
    }.to_string()
}

pub struct ClockConfig {
    pub time_step: f32,
    pub show_seconds: bool,
    pub show_date_cycle: bool,
    pub twelve_hour: bool,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            time_step: 0.1,
            show_seconds: true,
            show_date_cycle: true,
            twelve_hour: false,
        }
    }
}

// Track state for transitions and effects
struct ClockState {
    last_time: String,
    transition_frame: usize,
    showing_date: bool,
    was_showing_date: bool,
    last_switch: std::time::Instant,
    cycling: bool,
    cycle_digit: usize,
    last_cycle: std::time::Instant,
}

// Compact 3-line digits (same as pomodoro)
const DIGITS: [[&str; 3]; 10] = [
    ["█▀█", "█ █", "▀▀▀"],  // 0
    [" ▀█", "  █", "  ▀"],  // 1
    ["▀▀█", "█▀▀", "▀▀▀"],  // 2
    ["▀▀█", " ▀█", "▀▀▀"],  // 3
    ["█ █", "▀▀█", "  ▀"],  // 4
    ["█▀▀", "▀▀█", "▀▀▀"],  // 5
    ["█▀▀", "█▀█", "▀▀▀"],  // 6
    ["▀▀█", "  █", "  ▀"],  // 7
    ["█▀█", "█▀█", "▀▀▀"],  // 8
    ["█▀█", "▀▀█", "▀▀▀"],  // 9
];

const COLON: [&str; 3] = [" ▄ ", " ▄ ", "   "];
const DASH: [&str; 3] = ["   ", "▀▀▀", "   "];
const DIGIT_WIDTH: usize = 3;
const COLON_WIDTH: usize = 3;
const DASH_WIDTH: usize = 3;
const SPACING: usize = 1;

#[inline]
fn draw_big_time(term: &mut Terminal, cx: usize, cy: usize, time_str: &str, color: Color,
                 state: &ClockState, old_time: &str, show_seconds: bool, cycling: bool, cycle_digit: usize) {
    // Calculate total width
    let mut total_width = 0;
    let mut first = true;
    for ch in time_str.chars() {
        if !first {
            total_width += SPACING;
        }
        total_width += match ch {
            ':' => COLON_WIDTH,
            '-' => DASH_WIDTH,
            _ => DIGIT_WIDTH,
        };
        first = false;
    }

    let start_x = cx.saturating_sub(total_width / 2);
    let mut x_pos = start_x;

    // Special case: show cycling digits during anti-poisoning
    if cycling {
        // Show all positions with the same digit
        let digit_count = if show_seconds || state.showing_date { 8 } else { 6 };
        x_pos = cx.saturating_sub((digit_count * (DIGIT_WIDTH + SPACING) - SPACING) / 2);
        
        for _ in 0..digit_count {
            for (row, line) in DIGITS[cycle_digit].iter().enumerate() {
                let y = (cy + row) as i32;
                for (col, pch) in line.chars().enumerate() {
                    if pch != ' ' {
                        term.set((x_pos + col) as i32, y, pch, Some(color), false);
                    }
                }
            }
            x_pos += DIGIT_WIDTH + SPACING;
        }
        return;
    }

    // Special case: show all 8s during date/time transition
    if state.transition_frame > 0 && state.showing_date != state.was_showing_date {
        // Show 8 8s in a row during transition
        let eight_str = if show_seconds || state.showing_date { "88888888" } else { "888888" };
        x_pos = cx.saturating_sub((eight_str.len() * (DIGIT_WIDTH + SPACING) - SPACING) / 2);
        
        for _ in eight_str.chars() {
            for (row, line) in DIGITS[8].iter().enumerate() {
                let y = (cy + row) as i32;
                for (col, pch) in line.chars().enumerate() {
                    if pch != ' ' {
                        term.set((x_pos + col) as i32, y, pch, Some(color), false);
                    }
                }
            }
            x_pos += DIGIT_WIDTH + SPACING;
        }
        return;
    }

    // Normal display
    for (i, ch) in time_str.chars().enumerate() {
        let (pattern, width) = match ch {
            ':' => (&COLON, COLON_WIDTH),
            '-' => (&DASH, DASH_WIDTH),
            _ => {
                // Check if digit changed and we're in transition
                let old_ch = old_time.chars().nth(i).unwrap_or(' ');
                if ch != old_ch && state.transition_frame > 0 && ch.is_digit(10) {
                    // Show 8 during transition
                    (&DIGITS[8], DIGIT_WIDTH)
                } else {
                    let digit = (ch as u8 - b'0') as usize;
                    (&DIGITS[digit.min(9)], DIGIT_WIDTH)
                }
            }
        };

        for (row, line) in pattern.iter().enumerate() {
            let y = (cy + row) as i32;
            for (col, pch) in line.chars().enumerate() {
                if pch != ' ' {
                    term.set((x_pos + col) as i32, y, pch, Some(color), false);
                }
            }
        }

        x_pos += width + SPACING;
    }
}

#[inline]
fn draw_date(term: &mut Terminal, cx: usize, y: usize, date_str: &str, color: Color) {
    let start_x = cx.saturating_sub(date_str.len() / 2);
    term.set_str(start_x as i32, y as i32, date_str, Some(color), false);
}

pub fn run(mut config: ClockConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut colors = ColorState::new(7); // Default to mono

    // Cached timezone (computed once)
    let tz_name = get_tz_abbrev();

    // Reusable string buffers
    let mut time_buf = String::with_capacity(8);
    let mut date_buf = String::with_capacity(16);

    // Cache layout values
    let (mut w, mut h) = term.size();
    let mut cx = w as usize / 2;
    let mut start_y = (h as usize).saturating_sub(5) / 2;

    // Cache colors
    let mut last_scheme = colors.scheme;
    let mut time_color = Color::Cyan;
    let mut date_color = Color::DarkGrey;

    // Initialize state
    let mut state = ClockState {
        last_time: String::new(),
        transition_frame: 0,
        showing_date: false,
        was_showing_date: false,
        last_switch: std::time::Instant::now(),
        cycling: false,
        cycle_digit: 0,
        last_cycle: std::time::Instant::now(),
    };

    const TRANSITION_FRAMES: usize = 2;

    loop {
        // Handle input
        if let Ok(Some((code, _mods))) = term.check_key() {
            if !colors.handle_key(code) {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        // Toggle date/time display
                        state.showing_date = !state.showing_date;
                        state.transition_frame = TRANSITION_FRAMES;
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        // Toggle 12/24 hour format
                        config.twelve_hour = !config.twelve_hour;
                        state.transition_frame = TRANSITION_FRAMES;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        // Toggle seconds display
                        config.show_seconds = !config.show_seconds;
                        state.transition_frame = TRANSITION_FRAMES;
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        // Toggle auto date/time cycling
                        config.show_date_cycle = !config.show_date_cycle;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        // Trigger anti-poisoning cycle
                        state.cycling = true;
                        state.cycle_digit = 0;
                        state.last_cycle = std::time::Instant::now();
                    }
                    _ => {}
                }
            }
        }

        // Handle resize
        if let Ok((new_w, new_h)) = size() {
            if new_w != w || new_h != h {
                w = new_w;
                h = new_h;
                term.resize(w, h);
                term.clear_screen()?;
                cx = w as usize / 2;
                start_y = (h as usize).saturating_sub(5) / 2;
            }
        }

        // Update colors only when scheme changes
        if colors.scheme != last_scheme {
            last_scheme = colors.scheme;
            if colors.is_mono() {
                time_color = Color::Cyan;
                date_color = Color::DarkGrey;
            } else {
                time_color = crate::colors::scheme_color(colors.scheme, 3, true).0;
                date_color = crate::colors::scheme_color(colors.scheme, 1, false).0;
            }
        }

        // Auto-switch between date and time
        if config.show_date_cycle && state.last_switch.elapsed().as_secs() >= 8 {
            state.showing_date = !state.showing_date;
            state.last_switch = std::time::Instant::now();
            state.transition_frame = TRANSITION_FRAMES;
        }

        // Format time into reused buffer
        let now = Local::now();
        time_buf.clear();
        use std::fmt::Write;
        
        if state.showing_date {
            // Show date instead of time
            let _ = write!(time_buf, "{:02}-{:02}-{:02}", 
                now.month(), now.day(), now.year() % 100);
        } else {
            // Show time
            let hour = if config.twelve_hour {
                let h = now.hour() % 12;
                if h == 0 { 12 } else { h }
            } else {
                now.hour()
            };
            
            if config.show_seconds {
                let _ = write!(time_buf, "{:02}:{:02}:{:02}",
                    hour, now.minute(), now.second());
            } else {
                let _ = write!(time_buf, "{:02}:{:02}", hour, now.minute());
            }
        }

        // Format date into reused buffer with unix timestamp
        date_buf.clear();
        let _ = write!(date_buf, "{:02}-{:02}-{:02} {} {}",
            now.day(), now.month(), now.year() % 100, tz_name, now.timestamp());

        // Update transition state
        if state.last_time != time_buf && !state.last_time.is_empty() {
            state.transition_frame = TRANSITION_FRAMES;
        }

        // Render
        term.clear();
        draw_big_time(&mut term, cx, start_y, &time_buf, time_color, &state, &state.last_time, 
                      config.show_seconds, state.cycling, state.cycle_digit);
        
        // Show inverse information below
        if state.showing_date {
            // When showing date, display time below (with timezone and unix timestamp)
            let hour = if config.twelve_hour {
                let h = now.hour() % 12;
                if h == 0 { 12 } else { h }
            } else {
                now.hour()
            };
            
            let time_info = if config.twelve_hour {
                format!("{:02}:{:02}:{:02} {} {} {}", hour, now.minute(), now.second(),
                    if now.hour() >= 12 { "PM" } else { "AM" }, tz_name, now.timestamp())
            } else {
                format!("{:02}:{:02}:{:02} {} {}", hour, now.minute(), now.second(), 
                    tz_name, now.timestamp())
            };
            
            let x = cx.saturating_sub(time_info.len() / 2);
            term.set_str(x as i32, (start_y + 4) as i32, &time_info, Some(date_color), false);
        } else {
            // When showing time, display date below
            draw_date(&mut term, cx, start_y + 4, &date_buf, date_color);
        }

        term.present()?;
        
        // Update cycling animation
        if state.cycling && state.last_cycle.elapsed().as_millis() >= 100 {
            state.cycle_digit += 1;
            if state.cycle_digit > 9 {
                state.cycling = false;
                state.cycle_digit = 0;
            }
            state.last_cycle = std::time::Instant::now();
        }
        
        // Update state
        state.last_time = time_buf.clone();
        if state.transition_frame > 0 {
            state.transition_frame -= 1;
        }
        state.was_showing_date = state.showing_date;
        
        term.sleep(config.time_step);
    }

    Ok(())
}
