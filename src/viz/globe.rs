//! Rotating 3D globe with network activity (eDEX-UI style)

use crate::config::FractalConfig;
use crate::net_geo::ConnectionTracker;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::event::KeyCode;
use crossterm::style::Color;
use rand::prelude::*;
use std::io;
use std::sync::LazyLock;

// Globe visualization static data
#[inline]
const fn deg_to_rad(lat: f32, lon: f32) -> (f32, f32) {
    const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
    (lat * DEG_TO_RAD, lon * DEG_TO_RAD)
}

/// Returns the shortest angular delta from `from` to `to`, in range -PI..PI.
#[inline]
fn shortest_angular_delta(from: f32, to: f32) -> f32 {
    let mut delta = to - from;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

/// Normalize an angle to the range [-PI, PI].
#[inline]
fn normalize_longitude(lon: f32) -> f32 {
    let normalized = lon.rem_euclid(std::f32::consts::TAU);
    if normalized > std::f32::consts::PI {
        normalized - std::f32::consts::TAU
    } else {
        normalized
    }
}

static GLOBE_CONTINENTS: LazyLock<Vec<Vec<(f32, f32)>>> = LazyLock::new(|| vec![
    // North America (41 points)
    vec![
        deg_to_rad(69.5, -90.5), deg_to_rad(67.1, -81.4), deg_to_rad(58.9, -94.7),
        deg_to_rad(51.2, -79.9), deg_to_rad(62.6, -77.4), deg_to_rad(58.2, -67.6),
        deg_to_rad(60.3, -64.6), deg_to_rad(53.3, -55.8), deg_to_rad(46.8, -71.1),
        deg_to_rad(49.2, -65.1), deg_to_rad(45.9, -59.8), deg_to_rad(39.2, -76.3),
        deg_to_rad(31.4, -81.3), deg_to_rad(25.2, -80.4), deg_to_rad(30.1, -84.1),
        deg_to_rad(27.8, -97.1), deg_to_rad(18.8, -95.9), deg_to_rad(21.5, -87.1),
        deg_to_rad(15.9, -88.9), deg_to_rad(15.3, -83.4), deg_to_rad(9.0, -82.2),
        deg_to_rad(11.1, -74.9), deg_to_rad(7.2, -80.9), deg_to_rad(19.3, -105.0),
        deg_to_rad(31.2, -113.1), deg_to_rad(23.4, -109.4), deg_to_rad(24.7, -112.2),
        deg_to_rad(40.3, -124.4), deg_to_rad(49.0, -122.8), deg_to_rad(58.1, -134.1),
        deg_to_rad(61.3, -150.6), deg_to_rad(54.4, -164.8), deg_to_rad(58.9, -157.0),
        deg_to_rad(61.5, -166.1), deg_to_rad(64.8, -160.8), deg_to_rad(65.7, -168.1),
        deg_to_rad(71.4, -156.6), deg_to_rad(67.4, -108.9), deg_to_rad(67.3, -96.1),
        deg_to_rad(71.9, -95.2), deg_to_rad(69.5, -90.5),
    ],
    // South America (22 points)
    vec![
        deg_to_rad(11.1, -74.9), deg_to_rad(10.7, -61.9), deg_to_rad(4.2, -51.3),
        deg_to_rad(-0.1, -50.4), deg_to_rad(-7.3, -34.7), deg_to_rad(-21.9, -40.9),
        deg_to_rad(-24.9, -47.6), deg_to_rad(-34.4, -53.8), deg_to_rad(-33.9, -58.4),
        deg_to_rad(-36.9, -56.8), deg_to_rad(-41.1, -65.1), deg_to_rad(-48.1, -66.0),
        deg_to_rad(-53.8, -71.0), deg_to_rad(-52.3, -74.9), deg_to_rad(-46.6, -75.6),
        deg_to_rad(-42.4, -72.7), deg_to_rad(-18.3, -70.4), deg_to_rad(-14.6, -76.0),
        deg_to_rad(-4.7, -81.4), deg_to_rad(3.8, -77.1), deg_to_rad(9.0, -79.1),
        deg_to_rad(11.1, -74.9),
    ],
    // Europe (39 points)
    vec![
        deg_to_rad(31.2, 29.7), deg_to_rad(31.2, 34.3), deg_to_rad(36.7, 36.2),
        deg_to_rad(36.7, 27.6), deg_to_rad(39.5, 26.2), deg_to_rad(41.5, 41.6),
        deg_to_rad(45.2, 36.7), deg_to_rad(47.3, 39.1), deg_to_rad(44.4, 33.9),
        deg_to_rad(46.6, 30.7), deg_to_rad(41.1, 28.8), deg_to_rad(40.3, 22.6),
        deg_to_rad(36.4, 23.2), deg_to_rad(45.6, 13.9), deg_to_rad(40.2, 18.5),
        deg_to_rad(37.9, 15.7), deg_to_rad(44.4, 8.9), deg_to_rad(36.0, -5.9),
        deg_to_rad(36.9, -8.9), deg_to_rad(43.0, -9.4), deg_to_rad(43.4, -1.9),
        deg_to_rad(48.7, -4.6), deg_to_rad(53.5, 8.1), deg_to_rad(57.1, 8.5),
        deg_to_rad(54.0, 10.9), deg_to_rad(54.4, 19.7), deg_to_rad(59.2, 23.3),
        deg_to_rad(60.0, 29.1), deg_to_rad(60.7, 21.3), deg_to_rad(65.1, 25.4),
        deg_to_rad(65.7, 22.2), deg_to_rad(55.4, 12.9), deg_to_rad(59.5, 10.4),
        deg_to_rad(58.6, 5.7), deg_to_rad(62.6, 5.9), deg_to_rad(69.8, 19.2),
        deg_to_rad(70.5, 31.3), deg_to_rad(69.3, 33.8), deg_to_rad(31.2, 29.7),
    ],
    // Africa (16 points)
    vec![
        deg_to_rad(29.9, 32.4), deg_to_rad(11.7, 42.7), deg_to_rad(10.6, 51.0),
        deg_to_rad(-4.7, 39.2), deg_to_rad(-14.7, 40.8), deg_to_rad(-19.8, 34.8),
        deg_to_rad(-24.1, 35.5), deg_to_rad(-32.8, 28.2), deg_to_rad(-34.8, 19.6),
        deg_to_rad(-18.1, 11.8), deg_to_rad(-10.7, 13.7), deg_to_rad(3.7, 9.4),
        deg_to_rad(6.3, 4.3), deg_to_rad(4.4, -8.0), deg_to_rad(14.7, -17.6),
        deg_to_rad(29.9, 32.4),
    ],
    // Asia (43 points)
    vec![
        deg_to_rad(77.0, 107.0), deg_to_rad(70.8, 131.3), deg_to_rad(69.4, 178.6),
        deg_to_rad(62.3, 179.2), deg_to_rad(59.9, 163.5), deg_to_rad(51.0, 156.8),
        deg_to_rad(56.8, 155.9), deg_to_rad(62.6, 164.5), deg_to_rad(54.7, 135.1),
        deg_to_rad(52.2, 141.4), deg_to_rad(39.8, 127.5), deg_to_rad(35.1, 129.1),
        deg_to_rad(40.9, 121.6), deg_to_rad(39.2, 118.0), deg_to_rad(37.5, 122.4),
        deg_to_rad(34.9, 119.2), deg_to_rad(28.2, 121.7), deg_to_rad(19.8, 105.9),
        deg_to_rad(13.4, 109.3), deg_to_rad(8.6, 105.2), deg_to_rad(13.4, 100.1),
        deg_to_rad(1.3, 104.2), deg_to_rad(22.8, 91.4), deg_to_rad(15.9, 80.3),
        deg_to_rad(8.0, 77.5), deg_to_rad(21.4, 72.6), deg_to_rad(30.3, 48.9),
        deg_to_rad(24.0, 51.8), deg_to_rad(26.4, 56.4), deg_to_rad(22.3, 59.8),
        deg_to_rad(12.6, 43.5), deg_to_rad(21.3, 39.1), deg_to_rad(69.3, 33.8),
        deg_to_rad(67.5, 41.1), deg_to_rad(66.6, 33.2), deg_to_rad(63.8, 37.0),
        deg_to_rad(68.6, 43.5), deg_to_rad(68.1, 68.5), deg_to_rad(71.0, 66.7),
        deg_to_rad(73.0, 69.9), deg_to_rad(66.2, 72.4), deg_to_rad(72.8, 74.7),
        deg_to_rad(77.0, 107.0),
    ],
    // Australia (20 points)
    vec![
        deg_to_rad(-13.8, 143.6), deg_to_rad(-26.1, 153.1), deg_to_rad(-37.4, 150.0),
        deg_to_rad(-38.0, 140.6), deg_to_rad(-34.4, 138.2), deg_to_rad(-35.3, 136.8),
        deg_to_rad(-32.9, 137.8), deg_to_rad(-34.9, 136.0), deg_to_rad(-31.5, 131.3),
        deg_to_rad(-34.2, 115.0), deg_to_rad(-21.8, 114.1), deg_to_rad(-19.7, 120.9),
        deg_to_rad(-14.2, 125.7), deg_to_rad(-15.0, 129.6), deg_to_rad(-11.1, 132.4),
        deg_to_rad(-11.9, 136.5), deg_to_rad(-15.0, 135.5), deg_to_rad(-17.7, 140.2),
        deg_to_rad(-11.0, 142.1), deg_to_rad(-13.8, 143.6),
    ],
    // Greenland (21 points)
    vec![
        deg_to_rad(83.5, -27.1), deg_to_rad(82.7, -20.8), deg_to_rad(82.0, -31.4),
        deg_to_rad(81.3, -12.2), deg_to_rad(80.2, -20.0), deg_to_rad(80.1, -17.7),
        deg_to_rad(76.6, -21.7), deg_to_rad(74.3, -19.4), deg_to_rad(70.2, -26.4),
        deg_to_rad(70.1, -22.3), deg_to_rad(65.5, -39.8), deg_to_rad(60.1, -43.4),
        deg_to_rad(63.6, -51.6), deg_to_rad(67.2, -54.0), deg_to_rad(69.9, -50.9),
        deg_to_rad(69.6, -54.7), deg_to_rad(70.6, -51.4), deg_to_rad(75.5, -58.6),
        deg_to_rad(78.0, -73.3), deg_to_rad(81.8, -62.7), deg_to_rad(83.5, -27.1),
    ],
    // Japan (8 points)
    vec![
        deg_to_rad(37.1, 141.0), deg_to_rad(33.5, 135.8), deg_to_rad(33.9, 131.0),
        deg_to_rad(31.4, 130.2), deg_to_rad(33.3, 129.4), deg_to_rad(38.2, 139.4),
        deg_to_rad(41.2, 140.3), deg_to_rad(37.1, 141.0),
    ],
    // UK/Ireland (6 points)
    vec![
        deg_to_rad(58.6, -3.0), deg_to_rad(51.3, 1.4), deg_to_rad(50.0, -5.2),
        deg_to_rad(54.0, -2.9), deg_to_rad(56.8, -6.1), deg_to_rad(58.6, -3.0),
    ],
    // Antarctica (22 points)
    vec![
        deg_to_rad(-64.2, -58.6), deg_to_rad(-68.0, -65.7), deg_to_rad(-73.7, -60.8),
        deg_to_rad(-79.2, -78.0), deg_to_rad(-83.2, -58.2), deg_to_rad(-80.3, -28.5),
        deg_to_rad(-78.1, -35.3), deg_to_rad(-70.9, -6.9), deg_to_rad(-65.8, 54.5),
        deg_to_rad(-72.3, 69.9), deg_to_rad(-66.2, 88.0), deg_to_rad(-65.3, 135.1),
        deg_to_rad(-71.7, 171.2), deg_to_rad(-80.9, 159.8), deg_to_rad(-84.7, 180.0),
        deg_to_rad(-90.0, 180.0), deg_to_rad(-90.0, -180.0), deg_to_rad(-84.1, -179.1),
        deg_to_rad(-85.0, -143.1), deg_to_rad(-76.9, -158.4), deg_to_rad(-73.9, -74.9),
        deg_to_rad(-64.2, -58.6),
    ],
]);

static GLOBE_CITIES: LazyLock<Vec<(f32, f32)>> = LazyLock::new(|| vec![
    // North America
    deg_to_rad(40.7, -74.0),   // New York
    deg_to_rad(34.1, -118.2),  // Los Angeles
    deg_to_rad(41.9, -87.6),   // Chicago
    deg_to_rad(29.8, -95.4),   // Houston
    deg_to_rad(33.4, -112.1),  // Phoenix
    deg_to_rad(37.8, -122.4),  // San Francisco
    deg_to_rad(47.6, -122.3),  // Seattle
    deg_to_rad(43.7, -79.4),   // Toronto
    deg_to_rad(45.5, -73.6),   // Montreal
    deg_to_rad(19.4, -99.1),   // Mexico City
    // South America
    deg_to_rad(-23.5, -46.6),  // Sao Paulo
    deg_to_rad(-22.9, -43.2),  // Rio de Janeiro
    deg_to_rad(-34.6, -58.4),  // Buenos Aires
    deg_to_rad(-33.4, -70.6),  // Santiago
    deg_to_rad(-12.0, -77.0),  // Lima
    deg_to_rad(4.7, -74.1),    // Bogota
    // Europe
    deg_to_rad(51.5, -0.1),    // London
    deg_to_rad(48.9, 2.3),     // Paris
    deg_to_rad(52.5, 13.4),    // Berlin
    deg_to_rad(41.9, 12.5),    // Rome
    deg_to_rad(40.4, -3.7),    // Madrid
    deg_to_rad(52.4, 4.9),     // Amsterdam
    deg_to_rad(59.9, 10.8),    // Oslo
    deg_to_rad(59.3, 18.1),    // Stockholm
    deg_to_rad(55.8, 37.6),    // Moscow
    deg_to_rad(50.1, 14.4),    // Prague
    deg_to_rad(48.2, 16.4),    // Vienna
    deg_to_rad(41.0, 29.0),    // Istanbul
    // Africa
    deg_to_rad(30.0, 31.2),    // Cairo
    deg_to_rad(-33.9, 18.4),   // Cape Town
    deg_to_rad(-1.3, 36.8),    // Nairobi
    deg_to_rad(6.5, 3.4),      // Lagos
    deg_to_rad(33.6, -7.6),    // Casablanca
    deg_to_rad(-26.2, 28.0),   // Johannesburg
    // Asia
    deg_to_rad(35.7, 139.7),   // Tokyo
    deg_to_rad(31.2, 121.5),   // Shanghai
    deg_to_rad(39.9, 116.4),   // Beijing
    deg_to_rad(22.3, 114.2),   // Hong Kong
    deg_to_rad(1.4, 103.8),    // Singapore
    deg_to_rad(37.6, 127.0),   // Seoul
    deg_to_rad(13.8, 100.5),   // Bangkok
    deg_to_rad(28.6, 77.2),    // Delhi
    deg_to_rad(19.1, 72.9),    // Mumbai
    deg_to_rad(25.0, 121.5),   // Taipei
    deg_to_rad(14.6, 121.0),   // Manila
    deg_to_rad(-6.2, 106.8),   // Jakarta
    deg_to_rad(25.3, 55.3),    // Dubai
    deg_to_rad(32.1, 34.8),    // Tel Aviv
    // Oceania
    deg_to_rad(-33.9, 151.2),  // Sydney
    deg_to_rad(-37.8, 145.0),  // Melbourne
    deg_to_rad(-36.8, 174.8),  // Auckland
    deg_to_rad(-27.5, 153.0),  // Brisbane
]);

/// Fetch user's location from IP geolocation service
///
/// Note: Uses HTTP instead of HTTPS as ip-api.com free tier only supports HTTP.
/// This is acceptable for non-sensitive location data used for visualization positioning.
/// For production use with sensitive data, consider a paid API with HTTPS support.
fn fetch_user_location() -> Option<(f32, f32)> {
    let resp = ureq::get("http://ip-api.com/json/?fields=lat,lon")
        .timeout(std::time::Duration::from_secs(3))
        .call()
        .ok()?;

    let body = resp.into_string().ok()?;

    let lat = body.split("\"lat\":").nth(1)?
        .split(&[',', '}'][..]).next()?
        .trim().parse::<f32>().ok()?;
    let lon = body.split("\"lon\":").nth(1)?
        .split(&[',', '}'][..]).next()?
        .trim().parse::<f32>().ok()?;

    Some((lat.to_radians(), lon.to_radians()))
}

/// Help text for globe visualization
const HELP: &str = "\
GLOBE
─────────────────
↑/k    Pan up
↓/j    Pan down
+/-    Zoom in/out
0      Reset zoom";

/// Run the globe visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng, geoip_db: Option<&std::path::Path>, default_tilt: f32) -> io::Result<()> {
    let mut state = VizState::new(config.time_step, HELP);

    let user_location: Option<(f32, f32)> = fetch_user_location();
    let base_rotation: f32 = user_location.map(|(_, lon)| -lon).unwrap_or(0.0);
    let mut tilt: f32 = user_location.map(|(lat, _)| -lat).unwrap_or(default_tilt);
    let mut view_offset: f32 = 0.0;

    let mut zoom_override: Option<f32> = None;
    let mut current_zoom: f32 = 1.0;

    let mut conn_tracker = ConnectionTracker::new(geoip_db);
    let use_real_connections = conn_tracker.has_database();

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut braille_w = init_w as usize * 2;
    let mut braille_h = init_h as usize * 4;
    let mut braille_dots: Vec<Vec<u8>> = vec![vec![0; braille_w]; braille_h];

    struct Blip { lat: f32, lon: f32, age: f32, max_age: f32 }
    struct Arc { lat1: f32, lon1: f32, lat2: f32, lon2: f32, progress: f32 }

    let mut blips: Vec<Blip> = Vec::new();
    let mut arcs: Vec<Arc> = Vec::new();
    let mut user_pulse: f32 = 0.0;

    const TRIG_SIZE: usize = 360;
    let sin_table: Vec<f32> = (0..TRIG_SIZE)
        .map(|i| ((i as f32 / TRIG_SIZE as f32) * std::f32::consts::TAU).sin())
        .collect();
    let cos_table: Vec<f32> = (0..TRIG_SIZE)
        .map(|i| ((i as f32 / TRIG_SIZE as f32) * std::f32::consts::TAU).cos())
        .collect();

    let fast_sin = |x: f32| -> f32 {
        let normalized = x.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let idx = (normalized * TRIG_SIZE as f32) as usize;
        sin_table[idx.min(TRIG_SIZE - 1)]
    };
    let fast_cos = |x: f32| -> f32 {
        let normalized = x.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let idx = (normalized * TRIG_SIZE as f32) as usize;
        cos_table[idx.min(TRIG_SIZE - 1)]
    };

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            braille_w = width as usize * 2;
            braille_h = height as usize * 4;
            braille_dots = vec![vec![0; braille_w]; braille_h];
        }

        let w = width as f32;
        let h = height as f32;
        let half_w = w / 2.0;
        let half_h = h / 2.0;
        let base_radius = (h * 1.8).min(w * 0.8) * 0.4;
        let radius = base_radius * current_zoom;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    tilt = (tilt + 0.05).min(std::f32::consts::FRAC_PI_2);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    tilt = (tilt - 0.05).max(-std::f32::consts::FRAC_PI_2);
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    let current = zoom_override.unwrap_or(1.0);
                    zoom_override = Some((current * 1.2).min(3.0));
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    let current = zoom_override.unwrap_or(1.0);
                    zoom_override = Some((current / 1.2).max(0.3));
                }
                KeyCode::Char('0') => {
                    zoom_override = None;
                }
                _ => {}
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        for row in &mut braille_dots {
            for cell in row {
                *cell = 0;
            }
        }

        // Calculate solar position for day/night
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let hours_utc = ((secs % 86400) as f32) / 3600.0;
        let solar_lon = ((12.0 - hours_utc) / 24.0) * std::f32::consts::TAU;

        let daylight_level = |lon: f32| -> f32 {
            let abs_delta = shortest_angular_delta(solar_lon, lon).abs();
            let day_edge = std::f32::consts::FRAC_PI_2;
            let night_edge = std::f32::consts::FRAC_PI_2 + 0.314;
            if abs_delta < day_edge {
                1.0
            } else if abs_delta > night_edge {
                0.0
            } else {
                1.0 - (abs_delta - day_edge) / (night_edge - day_edge)
            }
        };

        let is_daylight = |lon: f32| -> bool { daylight_level(lon) > 0.5 };

        let rotation = base_rotation + view_offset;
        let (cos_tilt, sin_tilt) = (fast_cos(tilt), fast_sin(tilt));

        let lat_lon_to_screen = |lat: f32, lon: f32| -> Option<(i32, i32, f32)> {
            let cos_lat = fast_cos(lat);
            let sin_lat = fast_sin(lat);
            let cos_lon = fast_cos(lon + rotation);
            let sin_lon = fast_sin(lon + rotation);

            let x = cos_lat * sin_lon;
            let y = cos_lat * cos_lon;
            let z = sin_lat;

            let y2 = y * cos_tilt - z * sin_tilt;
            let z2 = y * sin_tilt + z * cos_tilt;

            if y2 < -0.1 {
                return None;
            }

            let screen_x = half_w + x * radius;
            let screen_y = half_h - z2 * radius * 0.5;

            let bx = (screen_x * 2.0) as i32;
            let by = (screen_y * 4.0) as i32;

            Some((bx, by, y2))
        };

        // Draw latitude lines
        for lat_deg in (-60..=60).step_by(30) {
            let lat = (lat_deg as f32).to_radians();
            for lon_deg in 0..360 {
                let lon = (lon_deg as f32).to_radians() - std::f32::consts::PI;
                if !is_daylight(lon) {
                    continue;
                }
                if let Some((bx, by, _)) = lat_lon_to_screen(lat, lon) {
                    if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32
                        && braille_dots[by as usize][bx as usize] == 0 {
                        braille_dots[by as usize][bx as usize] = 1;
                    }
                }
            }
        }

        // Draw longitude lines
        for lon_deg in (0..360).step_by(30) {
            let lon = (lon_deg as f32).to_radians() - std::f32::consts::PI;
            if !is_daylight(lon) {
                continue;
            }
            for lat_deg in -90..=90 {
                let lat = (lat_deg as f32).to_radians();
                if let Some((bx, by, _)) = lat_lon_to_screen(lat, lon) {
                    if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32
                        && braille_dots[by as usize][bx as usize] == 0 {
                        braille_dots[by as usize][bx as usize] = 1;
                    }
                }
            }
        }

        // Draw continents
        for continent in GLOBE_CONTINENTS.iter() {
            for i in 0..continent.len() {
                let (lat1, lon1) = continent[i];
                let (lat2, lon2) = continent[(i + 1) % continent.len()];

                for t in 0..20 {
                    let frac = t as f32 / 20.0;
                    let lat = lat1 + (lat2 - lat1) * frac;
                    let lon = lon1 + (lon2 - lon1) * frac;

                    if let Some((bx, by, _)) = lat_lon_to_screen(lat, lon) {
                        if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32 {
                            let dl = daylight_level(lon);
                            let intensity = if dl > 0.7 { 2 } else { 1 };
                            braille_dots[by as usize][bx as usize] = intensity;
                        }
                    }
                }
            }
        }

        // Handle network connections
        if use_real_connections {
            let _ = conn_tracker.update();
            let locations = conn_tracker.aggregated_locations(50);

            if !locations.is_empty() {
                if let Some((_, user_lon)) = user_location {
                    let mut max_angular_dist: f32 = 0.0;
                    let mut farthest_delta: f32 = 0.0;
                    for loc in &locations {
                        let delta = shortest_angular_delta(user_lon, loc.lon);
                        let angular_dist = delta.abs();
                        if angular_dist > max_angular_dist {
                            max_angular_dist = angular_dist;
                            farthest_delta = delta;
                        }
                    }
                    let target_offset = if max_angular_dist > std::f32::consts::FRAC_PI_2 * 0.9 {
                        let overshoot = max_angular_dist - std::f32::consts::FRAC_PI_2 * 0.8;
                        if farthest_delta > 0.0 { -overshoot } else { overshoot }
                    } else {
                        0.0
                    };
                    view_offset = view_offset * 0.9 + target_offset * 0.1;
                }
            }

            if zoom_override.is_none() && !locations.is_empty() {
                let mut all_lats: Vec<f32> = locations.iter().map(|l| l.lat).collect();
                let mut all_lons: Vec<f32> = locations.iter().map(|l| l.lon).collect();
                if let Some((user_lat, user_lon)) = user_location {
                    all_lats.push(user_lat);
                    all_lons.push(user_lon);
                }

                let lat_min = all_lats.iter().cloned().fold(f32::INFINITY, f32::min);
                let lat_max = all_lats.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let lon_min = all_lons.iter().cloned().fold(f32::INFINITY, f32::min);
                let lon_max = all_lons.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

                // Values are already in radians from user_location and GeoLocation
                let lat_span = (lat_max - lat_min).abs();
                let lon_span = (lon_max - lon_min).abs();
                let max_span = lat_span.max(lon_span).max(0.1);
                let target_zoom = (1.0 / max_span).clamp(0.5, 2.5);

                current_zoom = current_zoom * 0.92 + target_zoom * 0.08;
            }

            if let Some(override_zoom) = zoom_override {
                current_zoom = current_zoom * 0.9 + override_zoom * 0.1;
            }

            for loc in &locations {
                let exists = blips.iter().any(|b| {
                    (b.lat - loc.lat).abs() < 0.05 && (b.lon - loc.lon).abs() < 0.05
                });
                if !exists {
                    blips.push(Blip { lat: loc.lat, lon: loc.lon, age: 0.0, max_age: 3.0 });

                    if let Some((user_lat, user_lon)) = user_location {
                        if loc.is_outbound {
                            arcs.push(Arc { lat1: user_lat, lon1: user_lon, lat2: loc.lat, lon2: loc.lon, progress: 0.0 });
                        } else {
                            arcs.push(Arc { lat1: loc.lat, lon1: loc.lon, lat2: user_lat, lon2: user_lon, progress: 0.0 });
                        }
                    }
                }
            }
        } else {
            if rng.gen_bool(0.15) {
                let city_idx = rng.gen_range(0..GLOBE_CITIES.len());
                let (lat, lon) = GLOBE_CITIES[city_idx];
                blips.push(Blip { lat, lon, age: 0.0, max_age: rng.gen_range(0.5..2.0) });
            }

            if rng.gen_bool(0.03) && blips.len() >= 2 {
                let i1 = rng.gen_range(0..blips.len());
                let i2 = rng.gen_range(0..blips.len());
                if i1 != i2 {
                    arcs.push(Arc {
                        lat1: blips[i1].lat, lon1: blips[i1].lon,
                        lat2: blips[i2].lat, lon2: blips[i2].lon,
                        progress: 0.0,
                    });
                }
            }

            if let Some(override_zoom) = zoom_override {
                current_zoom = current_zoom * 0.9 + override_zoom * 0.1;
            }
        }

        // Draw and update blips
        let mut new_blips = Vec::new();
        for mut blip in blips {
            blip.age += state.speed * 2.0;
            if blip.age < blip.max_age {
                let pulse = (blip.age / blip.max_age * std::f32::consts::PI).sin();
                let size = (pulse * 3.0) as i32;

                if let Some((bx, by, _)) = lat_lon_to_screen(blip.lat, blip.lon) {
                    for dy in -size..=size {
                        for dx in -size..=size {
                            let px = bx + dx;
                            let py = by + dy;
                            if px >= 0 && px < braille_w as i32 && py >= 0 && py < braille_h as i32 {
                                braille_dots[py as usize][px as usize] = 3;
                            }
                        }
                    }
                }
                new_blips.push(blip);
            }
        }
        blips = new_blips;

        // Draw and update arcs
        let mut new_arcs = Vec::new();
        for mut arc in arcs {
            arc.progress += state.speed * 1.5;
            if arc.progress < 1.0 {
                let steps = (arc.progress * 30.0) as i32;
                for t in 0..=steps {
                    let frac = t as f32 / 30.0;
                    let lat = arc.lat1 + (arc.lat2 - arc.lat1) * frac;
                    // Interpolate longitude using shortest path and normalize to [-PI, PI]
                    let lon = normalize_longitude(arc.lon1 + shortest_angular_delta(arc.lon1, arc.lon2) * frac);
                    let arc_height = (frac * std::f32::consts::PI).sin() * 0.1;
                    let lat_adj = lat + arc_height;

                    if let Some((bx, by, _)) = lat_lon_to_screen(lat_adj, lon) {
                        if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32 {
                            braille_dots[by as usize][bx as usize] = 3;
                        }
                    }
                }
                new_arcs.push(arc);
            }
        }
        arcs = new_arcs;

        // Draw user location marker
        if let Some((user_lat, user_lon)) = user_location {
            user_pulse += state.speed * 3.0;
            let pulse_size = ((user_pulse.sin() + 1.0) * 2.0 + 2.0) as i32;

            if let Some((bx, by, _)) = lat_lon_to_screen(user_lat, user_lon) {
                for dy in -pulse_size..=pulse_size {
                    for dx in -pulse_size..=pulse_size {
                        if dx.abs() + dy.abs() <= pulse_size {
                            let px = bx + dx;
                            let py = by + dy;
                            if px >= 0 && px < braille_w as i32 && py >= 0 && py < braille_h as i32 {
                                braille_dots[py as usize][px as usize] = 4;
                            }
                        }
                    }
                }
            }
        }

        // Render braille to terminal
        term.clear();
        for cy in 0..height as usize {
            let by = cy * 4;
            if by + 3 >= braille_h {
                continue;
            }
            for cx in 0..width as usize {
                let bx = cx * 2;
                if bx + 1 >= braille_w {
                    continue;
                }

                let mut dots: u8 = 0;
                let mut max_intensity: u8 = 0;

                let positions = [
                    (by, bx), (by + 1, bx), (by + 2, bx),
                    (by, bx + 1), (by + 1, bx + 1), (by + 2, bx + 1),
                    (by + 3, bx), (by + 3, bx + 1),
                ];
                let dot_bits = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

                for (i, &(py, px)) in positions.iter().enumerate() {
                    let val = braille_dots[py][px];
                    if val > 0 {
                        dots |= dot_bits[i];
                        max_intensity = max_intensity.max(val);
                    }
                }

                if dots > 0 {
                    let ch = char::from_u32(0x2800 + dots as u32).unwrap_or(' ');
                    let (color, bold) = if max_intensity == 4 {
                        (Color::Yellow, true)
                    } else {
                        let intensity = match max_intensity {
                            1 => 0,
                            2 => 2,
                            _ => 3,
                        };
                        scheme_color(state.color_scheme(), intensity, max_intensity >= 3)
                    };
                    term.set(cx as i32, cy as i32, ch, Some(color), bold);
                }
            }
        }

        state.render_help(term, width, height);
        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
