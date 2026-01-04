use crate::colors::{ColorState, scheme_color};
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use rand::Rng;
use serde::Deserialize;
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    HeavyRain,
    Snow,
    HeavySnow,
    Thunderstorm,
}

impl WeatherCondition {
    fn from_code(code: i32, _is_day: bool) -> Self {
        match code {
            0 => WeatherCondition::Clear,
            1 | 2 => WeatherCondition::PartlyCloudy,
            3 => WeatherCondition::Cloudy,
            45 | 48 => WeatherCondition::Fog,
            51 | 53 | 55 => WeatherCondition::Drizzle,
            61 | 63 | 80 | 81 => WeatherCondition::Rain,
            65 | 82 => WeatherCondition::HeavyRain,
            71 | 73 | 77 | 85 => WeatherCondition::Snow,
            75 | 86 => WeatherCondition::HeavySnow,
            95 | 96 | 99 => WeatherCondition::Thunderstorm,
            _ => WeatherCondition::Clear,
        }
    }

    fn description(&self) -> &'static str {
        match self {
            WeatherCondition::Clear => "Clear",
            WeatherCondition::PartlyCloudy => "Partly Cloudy",
            WeatherCondition::Cloudy => "Cloudy",
            WeatherCondition::Fog => "Foggy",
            WeatherCondition::Drizzle => "Drizzle",
            WeatherCondition::Rain => "Rainy",
            WeatherCondition::HeavyRain => "Heavy Rain",
            WeatherCondition::Snow => "Snowy",
            WeatherCondition::HeavySnow => "Heavy Snow",
            WeatherCondition::Thunderstorm => "Thunderstorm",
        }
    }
}

#[derive(Debug, Deserialize)]
struct GeoResponse {
    latitude: f64,
    longitude: f64,
    city: Option<String>,
    region: Option<String>,
    #[allow(dead_code)]
    country_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    winddirection: f64,
    weathercode: i32,
    is_day: i32,
    time: String,  // ISO8601 format: "2025-12-13T16:30"
}

#[derive(Debug, Deserialize)]
struct DailyWeather {
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current_weather: CurrentWeather,
    daily: Option<DailyWeather>,
}

pub struct WeatherData {
    pub temperature: f64,
    pub temp_high: f64,
    pub temp_low: f64,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub condition: WeatherCondition,
    pub is_day: bool,
    pub location: String,
    pub observation_time: String,  // When weather was observed
}

struct Particle {
    x: f32,
    y: f32,
    speed: f32,
    char: char,
}

pub struct WeatherDisplay {
    data: Option<WeatherData>,
    particles: Vec<Particle>,
    frame: usize,
    lightning_flash: usize,
    error: Option<String>,
}

impl WeatherDisplay {
    pub fn new() -> Self {
        Self {
            data: None,
            particles: Vec::new(),
            frame: 0,
            lightning_flash: 0,
            error: None,
        }
    }

    pub fn fetch_weather(&mut self, location: Option<&str>) -> io::Result<()> {
        // Get coordinates
        let (lat, lon, loc_name) = if let Some(loc) = location {
            // Try to geocode the location using Open-Meteo geocoding API
            match self.geocode_location(loc) {
                Ok((lat, lon, name)) => (lat, lon, name),
                Err(e) => {
                    self.error = Some(format!("Geocoding failed: {}", e));
                    return Ok(());
                }
            }
        } else {
            // Auto-detect from IP
            match self.get_location_from_ip() {
                Ok((lat, lon, name)) => (lat, lon, name),
                Err(e) => {
                    self.error = Some(format!("Location detection failed: {}", e));
                    return Ok(());
                }
            }
        };

        // Fetch weather data (current + today's high/low)
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true&daily=temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=1",
            lat, lon
        );

        match ureq::get(&url).call() {
            Ok(response) => {
                match response.into_json::<WeatherResponse>() {
                    Ok(weather) => {
                        let cw = weather.current_weather;
                        let (high, low) = weather.daily
                            .map(|d| {
                                let h = d.temperature_2m_max.first().copied().unwrap_or(cw.temperature);
                                let l = d.temperature_2m_min.first().copied().unwrap_or(cw.temperature);
                                (h, l)
                            })
                            .unwrap_or((cw.temperature, cw.temperature));
                        self.data = Some(WeatherData {
                            temperature: cw.temperature,
                            temp_high: high,
                            temp_low: low,
                            wind_speed: cw.windspeed,
                            wind_direction: cw.winddirection,
                            condition: WeatherCondition::from_code(cw.weathercode, cw.is_day == 1),
                            is_day: cw.is_day == 1,
                            location: loc_name,
                            observation_time: cw.time,
                        });
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(format!("Parse error: {}", e));
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Network error: {}", e));
            }
        }

        Ok(())
    }

    fn get_location_from_ip(&self) -> Result<(f64, f64, String), String> {
        let response = ureq::get("https://ipapi.co/json/")
            .call()
            .map_err(|e| e.to_string())?;

        let geo: GeoResponse = response.into_json().map_err(|e| e.to_string())?;

        let name = format!(
            "{}{}",
            geo.city.unwrap_or_default(),
            geo.region.map(|r| format!(", {}", r)).unwrap_or_default()
        );

        Ok((geo.latitude, geo.longitude, name))
    }

    fn geocode_location(&self, query: &str) -> Result<(f64, f64, String), String> {
        let url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1",
            urlencoding(query)
        );

        #[derive(Deserialize)]
        struct GeoResult {
            name: String,
            latitude: f64,
            longitude: f64,
            #[allow(dead_code)]
            country: Option<String>,
            admin1: Option<String>,
        }

        #[derive(Deserialize)]
        struct GeoResults {
            results: Option<Vec<GeoResult>>,
        }

        let response = ureq::get(&url)
            .call()
            .map_err(|e| e.to_string())?;

        let results: GeoResults = response.into_json().map_err(|e| e.to_string())?;

        if let Some(results) = results.results {
            if let Some(first) = results.into_iter().next() {
                let name = format!(
                    "{}{}",
                    first.name,
                    first.admin1.map(|a| format!(", {}", a)).unwrap_or_default()
                );
                return Ok((first.latitude, first.longitude, name));
            }
        }

        Err("Location not found".to_string())
    }

    pub fn init_particles(&mut self, w: usize, h: usize) {
        self.particles.clear();

        let Some(ref data) = self.data else { return };

        let mut rng = rand::thread_rng();
        let count = match data.condition {
            WeatherCondition::Drizzle => 30,
            WeatherCondition::Rain => 80,
            WeatherCondition::HeavyRain => 150,
            WeatherCondition::Snow => 50,
            WeatherCondition::HeavySnow => 100,
            WeatherCondition::Thunderstorm => 120,
            _ => 0,
        };

        let (ch, speed_range) = match data.condition {
            WeatherCondition::Snow | WeatherCondition::HeavySnow => ('*', (0.3, 0.8)),
            _ => ('|', (1.0, 2.5)),
        };

        for _ in 0..count {
            self.particles.push(Particle {
                x: rng.gen_range(0.0..w as f32),
                y: rng.gen_range(0.0..h as f32),
                speed: rng.gen_range(speed_range.0..speed_range.1),
                char: ch,
            });
        }
    }

    pub fn update(&mut self, w: usize, h: usize) {
        self.frame = self.frame.wrapping_add(1);

        let Some(ref data) = self.data else { return };

        let mut rng = rand::thread_rng();

        // Update particles
        let is_snow = matches!(data.condition, WeatherCondition::Snow | WeatherCondition::HeavySnow);

        for p in &mut self.particles {
            p.y += p.speed;

            if is_snow {
                // Snow drifts sideways
                p.x += rng.gen_range(-0.3..0.3);
            }

            // Wrap around
            if p.y >= h as f32 {
                p.y = 0.0;
                p.x = rng.gen_range(0.0..w as f32);
            }
            if p.x < 0.0 { p.x = w as f32 - 1.0; }
            if p.x >= w as f32 { p.x = 0.0; }
        }

        // Lightning for thunderstorms
        if matches!(data.condition, WeatherCondition::Thunderstorm) {
            if self.lightning_flash > 0 {
                self.lightning_flash -= 1;
            } else if rng.gen_ratio(1, 60) {
                self.lightning_flash = rng.gen_range(2..6);
            }
        }
    }

    pub fn render(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState, use_fahrenheit: bool) {
        let Some(ref data) = self.data else {
            if let Some(ref err) = self.error {
                let cy = h / 2;
                let msg = format!("Error: {}", err);
                let cx = w.saturating_sub(msg.len()) / 2;
                let c = if colors.is_mono() { Color::Red } else { scheme_color(colors.scheme, 3, true).0 };
                term.set_str(cx as i32, cy as i32, &msg, Some(c), false);
            } else {
                let cy = h / 2;
                let msg = "Fetching weather...";
                let cx = w.saturating_sub(msg.len()) / 2;
                let c = if colors.is_mono() { Color::Yellow } else { scheme_color(colors.scheme, 2, false).0 };
                term.set_str(cx as i32, cy as i32, msg, Some(c), false);
            }
            return;
        };

        // Background color based on conditions
        let bg_flash = self.lightning_flash > 0 && self.lightning_flash % 2 == 0;

        // Draw sky gradient
        self.draw_sky(term, w, h, data, bg_flash, colors);

        // Draw weather-specific elements
        match data.condition {
            WeatherCondition::Clear => self.draw_clear(term, w, h, data, colors),
            WeatherCondition::PartlyCloudy => self.draw_partly_cloudy(term, w, h, data, colors),
            WeatherCondition::Cloudy => self.draw_cloudy(term, w, h, colors),
            WeatherCondition::Fog => self.draw_fog(term, w, h, colors),
            WeatherCondition::Drizzle | WeatherCondition::Rain | WeatherCondition::HeavyRain => {
                self.draw_rain_clouds(term, w, h, colors);
            }
            WeatherCondition::Snow | WeatherCondition::HeavySnow => {
                self.draw_snow_clouds(term, w, h, colors);
            }
            WeatherCondition::Thunderstorm => {
                self.draw_storm_clouds(term, w, h, bg_flash, colors);
            }
        }

        // Draw particles (rain/snow)
        self.draw_particles(term, data, colors);

        // Draw info panel
        self.draw_info(term, w, h, data, colors, use_fahrenheit);
    }

    fn draw_sky(&self, term: &mut Terminal, w: usize, h: usize, data: &WeatherData, flash: bool, colors: &ColorState) {
        // Fog obscures the sky completely
        if data.condition == WeatherCondition::Fog {
            return;
        }

        let color = if flash {
            Color::White
        } else if colors.is_mono() {
            if data.is_day {
                match data.condition {
                    WeatherCondition::Clear | WeatherCondition::PartlyCloudy => Color::Cyan,
                    WeatherCondition::Cloudy => Color::Grey,
                    _ => Color::DarkGrey,
                }
            } else {
                Color::DarkBlue
            }
        } else {
            scheme_color(colors.scheme, 0, false).0
        };

        // Fill with sky color using faint dots
        for y in 0..h/2 {
            for x in 0..w {
                if (x + y) % 4 == 0 {
                    term.set(x as i32, y as i32, '·', Some(color), false);
                }
            }
        }
    }

    fn draw_clear(&self, term: &mut Terminal, w: usize, h: usize, data: &WeatherData, colors: &ColorState) {
        let cx = w / 2;
        let cy = h / 4;

        if data.is_day {
            // Animated sun
            let sun = [
                r"    \   |   /    ",
                r"      .---.      ",
                r"   -- (   ) --   ",
                r"      `---'      ",
                r"    /   |   \    ",
            ];

            let sun_color = if colors.is_mono() { Color::Yellow } else { scheme_color(colors.scheme, 3, true).0 };
            let start_y = cy.saturating_sub(2);
            for (i, line) in sun.iter().enumerate() {
                let start_x = cx.saturating_sub(line.len() / 2);
                term.set_str(start_x as i32, (start_y + i) as i32, line, Some(sun_color), true);
            }
        } else {
            // Just stars for nighttime - no moon
            // Stable star field with twinkling effect
            let star_color = if colors.is_mono() { Color::White } else { scheme_color(colors.scheme, 2, false).0 };
            let star_positions = [
                (w/8, h/8), (w/6, h/5), (w/4, h/10), (w/3, h/7),
                (2*w/5, h/9), (w/2, h/6), (3*w/5, h/11), (2*w/3, h/8),
                (3*w/4, h/5), (5*w/6, h/9), (7*w/8, h/7), (w/10, h/4),
                (9*w/10, h/10), (w/7, h/3), (4*w/5, h/12)
            ];
            
            for &(sx, sy) in &star_positions {
                if sx < w && sy < h/2 {
                    // Twinkle based on position and frame to create shimmer
                    let twinkle = ((self.frame / 3 + sx + sy) % 17) < 12;
                    let star = if twinkle { '✦' } else { '·' };
                    let brightness = if twinkle {
                        star_color
                    } else if colors.is_mono() {
                        Color::Grey
                    } else {
                        scheme_color(colors.scheme, 1, false).0
                    };
                    term.set(sx as i32, sy as i32, star, Some(brightness), false);
                }
            }
        }
    }

    fn draw_partly_cloudy(&self, term: &mut Terminal, w: usize, h: usize, data: &WeatherData, colors: &ColorState) {
        // Draw sun/moon first
        self.draw_clear(term, w, h, data, colors);

        // Then draw some clouds
        let cloud = [
            r"     .--.    ",
            r"  .-(    ). ",
            r" (___.__)__)",
        ];

        let cloud_color = if colors.is_mono() { Color::White } else { scheme_color(colors.scheme, 2, false).0 };

        // Cloud positions (animated drift)
        let drift = (self.frame / 4) % w;
        let positions = [(w/4 + drift) % w, (3*w/4 + drift/2) % w];

        for (i, &px) in positions.iter().enumerate() {
            let py = h/3 + i * 2;
            for (j, line) in cloud.iter().enumerate() {
                let x = px.saturating_sub(line.len() / 2);
                term.set_str(x as i32, (py + j) as i32, line, Some(cloud_color), false);
            }
        }
    }

    fn draw_cloudy(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        let clouds = [
            (r"      .--.      ", r"   .-(    ).   ", r"  (___.__)__)  "),
            (r"    .--.    ", r" .-(    ). ", r"(___.__)__)"),
        ];

        let cloud_top = if colors.is_mono() { Color::White } else { scheme_color(colors.scheme, 2, false).0 };
        let cloud_bottom = if colors.is_mono() { Color::Grey } else { scheme_color(colors.scheme, 1, false).0 };

        let drift = (self.frame / 6) % w;
        let positions = [
            (w/5 + drift, h/4),
            (2*w/5 + drift/2, h/3),
            (3*w/5 + drift, h/4 + 1),
            (4*w/5 + drift/3, h/3 + 1),
        ];

        for (i, &(px, py)) in positions.iter().enumerate() {
            let cloud = &clouds[i % 2];
            let x = (px % w).saturating_sub(cloud.0.len() / 2);
            term.set_str(x as i32, py as i32, cloud.0, Some(cloud_top), false);
            term.set_str(x as i32, (py + 1) as i32, cloud.1, Some(cloud_top), false);
            term.set_str(x as i32, (py + 2) as i32, cloud.2, Some(cloud_bottom), false);
        }
    }

    fn draw_fog(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        let fog_chars = ['~', '-', '~', '_', '-'];
        let fog_color = if colors.is_mono() { Color::Grey } else { scheme_color(colors.scheme, 1, false).0 };
        let mut rng = rand::thread_rng();

        for y in h/3..2*h/3 {
            for x in 0..w {
                if rng.gen_ratio(1, 3) {
                    let ch = fog_chars[(x + y + self.frame/2) % fog_chars.len()];
                    term.set(x as i32, y as i32, ch, Some(fog_color), false);
                }
            }
        }
    }

    fn draw_rain_clouds(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        let cloud = [
            r"      .--.      ",
            r"   .-(    ).   ",
            r"  (___.__)__)  ",
        ];

        let cloud_color = if colors.is_mono() { Color::DarkGrey } else { scheme_color(colors.scheme, 1, false).0 };
        let positions = [(w/4, h/5), (w/2, h/6), (3*w/4, h/5)];

        for &(px, py) in &positions {
            let x = px.saturating_sub(cloud[0].len() / 2);
            term.set_str(x as i32, py as i32, cloud[0], Some(cloud_color), false);
            term.set_str(x as i32, (py + 1) as i32, cloud[1], Some(cloud_color), false);
            term.set_str(x as i32, (py + 2) as i32, cloud[2], Some(cloud_color), false);
        }
    }

    fn draw_snow_clouds(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        // Same as rain but lighter
        self.draw_rain_clouds(term, w, h, colors);
    }

    fn draw_storm_clouds(&self, term: &mut Terminal, w: usize, h: usize, flash: bool, colors: &ColorState) {
        let cloud = [
            r"      .--.      ",
            r"   .-(    ).   ",
            r"  (___.__)__)  ",
        ];

        let color = if flash {
            Color::White
        } else if colors.is_mono() {
            Color::DarkGrey
        } else {
            scheme_color(colors.scheme, 1, false).0
        };
        let positions = [(w/4, h/6), (w/2, h/7), (3*w/4, h/6)];

        for &(px, py) in &positions {
            let x = px.saturating_sub(cloud[0].len() / 2);
            term.set_str(x as i32, py as i32, cloud[0], Some(color), false);
            term.set_str(x as i32, (py + 1) as i32, cloud[1], Some(color), false);
            term.set_str(x as i32, (py + 2) as i32, cloud[2], Some(color), false);
        }

        // Lightning bolt
        if flash {
            let bolt = ["/", "\\", "/", "|", "\\", "/"];
            let bx = w / 2;
            let by = h / 4;
            for (i, &ch) in bolt.iter().enumerate() {
                term.set_str((bx + i % 2) as i32, (by + i) as i32, ch, Some(Color::Yellow), true);
            }
        }
    }

    fn draw_particles(&self, term: &mut Terminal, data: &WeatherData, colors: &ColorState) {
        let color = if colors.is_mono() {
            match data.condition {
                WeatherCondition::Snow | WeatherCondition::HeavySnow => Color::White,
                _ => Color::Cyan,
            }
        } else {
            scheme_color(colors.scheme, 2, false).0
        };

        for p in &self.particles {
            term.set(p.x as i32, p.y as i32, p.char, Some(color), false);
        }
    }

    fn draw_info(&self, term: &mut Terminal, _w: usize, h: usize, data: &WeatherData, colors: &ColorState, use_fahrenheit: bool) {
        // Info panel at bottom
        let panel_y = h - 6;

        // Location
        let loc = &data.location;
        let loc_color = if colors.is_mono() { Color::White } else { scheme_color(colors.scheme, 3, true).0 };
        term.set_str(2, panel_y as i32, loc, Some(loc_color), true);

        // Helper to convert C to F
        let to_display = |c: f64| -> i32 {
            if use_fahrenheit {
                (c * 9.0 / 5.0 + 32.0).round() as i32
            } else {
                c.round() as i32
            }
        };
        let unit = if use_fahrenheit { "°F" } else { "°C" };

        // Current temperature
        let temp = format!("{}{}", to_display(data.temperature), unit);
        let temp_color = if colors.is_mono() {
            // Semantic temperature colors
            if data.temperature < 0.0 {
                Color::Cyan
            } else if data.temperature < 10.0 {
                Color::Blue
            } else if data.temperature < 20.0 {
                Color::Green
            } else if data.temperature < 30.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        } else {
            scheme_color(colors.scheme, 3, true).0
        };
        term.set_str(2, (panel_y + 1) as i32, &temp, Some(temp_color), true);

        // High/Low
        let hi_lo = format!("H:{}{}  L:{}{}", to_display(data.temp_high), unit, to_display(data.temp_low), unit);
        let hi_lo_color = if colors.is_mono() { Color::Grey } else { scheme_color(colors.scheme, 1, false).0 };
        term.set_str(2, (panel_y + 2) as i32, &hi_lo, Some(hi_lo_color), false);

        // Condition
        let cond_color = if colors.is_mono() { Color::Grey } else { scheme_color(colors.scheme, 1, false).0 };
        term.set_str(2, (panel_y + 3) as i32, data.condition.description(), Some(cond_color), false);

        // Wind
        let wind = format!("Wind: {} km/h", data.wind_speed.round() as i32);
        let wind_color = if colors.is_mono() { Color::DarkGrey } else { scheme_color(colors.scheme, 0, false).0 };
        term.set_str(2, (panel_y + 4) as i32, &wind, Some(wind_color), false);

        // Wind direction arrow
        let arrow = match data.wind_direction as i32 {
            0..=22 | 338..=360 => "↑",
            23..=67 => "↗",
            68..=112 => "→",
            113..=157 => "↘",
            158..=202 => "↓",
            203..=247 => "↙",
            248..=292 => "←",
            293..=337 => "↖",
            _ => "?",
        };
        term.set_str((2 + wind.len() + 1) as i32, (panel_y + 4) as i32, arrow, Some(wind_color), false);

        // Observation time at bottom
        let obs_time = parse_observation_time(&data.observation_time);
        let time_str = format!("Updated {}", obs_time);
        let time_color = if colors.is_mono() { Color::DarkGrey } else { scheme_color(colors.scheme, 0, false).0 };
        term.set_str(2, (panel_y + 5) as i32, &time_str, Some(time_color), false);
    }
}

// Simple URL encoding for location queries
/// Parse ISO8601 time "2025-12-13T16:30" into "4:30 PM"
fn parse_observation_time(iso_time: &str) -> String {
    // Extract time part after 'T'
    if let Some(time_part) = iso_time.split('T').nth(1) {
        let parts: Vec<&str> = time_part.split(':').collect();
        if parts.len() >= 2 {
            if let Ok(hour) = parts[0].parse::<u32>() {
                let minute = parts[1];
                let (hour12, ampm) = if hour == 0 {
                    (12, "AM")
                } else if hour < 12 {
                    (hour, "AM")
                } else if hour == 12 {
                    (12, "PM")
                } else {
                    (hour - 12, "PM")
                };
                return format!("{}:{} {}", hour12, minute, ampm);
            }
        }
    }
    iso_time.to_string() // Fallback to raw string
}

fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                for b in c.to_string().bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

pub struct WeatherConfig {
    pub location: Option<String>,
    pub time_step: f32,
}

pub fn run(config: WeatherConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut display = WeatherDisplay::new();
    let mut colors = ColorState::new(7); // Default to mono (semantic colors)
    let mut use_fahrenheit = true; // Default to Fahrenheit

    // Fetch weather data
    display.fetch_weather(config.location.as_deref())?;

    // Initialize particles based on terminal size
    let (w, h) = term.size();
    display.init_particles(w as usize, h as usize);

    // Auto-refresh every 20 minutes
    let refresh_interval = Duration::from_secs(20 * 60);
    let mut last_refresh = Instant::now();

    loop {
        // Check for quit
        if let Ok(Some((code, _))) = term.check_key() {
            if !colors.handle_key(code) {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => {
                        // Manual refresh
                        display.fetch_weather(config.location.as_deref())?;
                        display.init_particles(w as usize, h as usize);
                        last_refresh = Instant::now();
                    }
                    KeyCode::Char('f') => use_fahrenheit = !use_fahrenheit,
                    _ => {}
                }
            }
        }

        // Auto-refresh weather data
        if last_refresh.elapsed() >= refresh_interval {
            let _ = display.fetch_weather(config.location.as_deref()); // Ignore errors on auto-refresh
            last_refresh = Instant::now();
        }

        // Handle resize
        if let Ok((new_w, new_h)) = size() {
            let (cur_w, cur_h) = term.size();
            if new_w != cur_w || new_h != cur_h {
                term.resize(new_w, new_h);
                term.clear_screen()?;
                display.init_particles(new_w as usize, new_h as usize);
            }
        }

        display.update(term.size().0 as usize, term.size().1 as usize);

        term.clear();
        let (w, h) = term.size();
        display.render(&mut term, w as usize, h as usize, &colors, use_fahrenheit);
        term.present()?;

        term.sleep(config.time_step);
    }

    Ok(())
}
