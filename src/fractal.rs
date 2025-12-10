use crate::config::FractalConfig;
use crate::terminal::Terminal;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Color;
use rand::prelude::*;
use std::io;

/// Runtime state for interactive controls
struct VizState {
    speed: f32,        // Current speed (time per frame)
    color_scheme: u8,  // Current color scheme (0-9)
    paused: bool,
}

impl VizState {
    fn new(initial_speed: f32) -> Self {
        Self {
            speed: initial_speed,
            color_scheme: 0,
            paused: false,
        }
    }

    /// Handle keypress, returns true if should quit
    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char(' ') => self.paused = !self.paused,
            // Number keys: change speed (1=fastest, 9=slowest, 0=very slow)
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as u8;
                self.speed = match n {
                    0 => 0.2,
                    1 => 0.005,
                    2 => 0.01,
                    3 => 0.02,
                    4 => 0.03,
                    5 => 0.05,
                    6 => 0.07,
                    7 => 0.1,
                    8 => 0.15,
                    9 => 0.2,
                    _ => self.speed,
                };
            }
            // Shift+number produces symbols - use these for color schemes
            KeyCode::Char('!') => self.color_scheme = 1,  // Shift+1: fire
            KeyCode::Char('@') => self.color_scheme = 2,  // Shift+2: ice
            KeyCode::Char('#') => self.color_scheme = 3,  // Shift+3: pink
            KeyCode::Char('$') => self.color_scheme = 4,  // Shift+4: gold
            KeyCode::Char('%') => self.color_scheme = 5,  // Shift+5: electric
            KeyCode::Char('^') => self.color_scheme = 6,  // Shift+6: lava
            KeyCode::Char('&') => self.color_scheme = 7,  // Shift+7: mono
            KeyCode::Char('*') => self.color_scheme = 8,  // Shift+8: rainbow
            KeyCode::Char('(') => self.color_scheme = 9,  // Shift+9: neon
            KeyCode::Char(')') => self.color_scheme = 0,  // Shift+0: green/matrix
            _ => {}
        }
        false
    }
}

/// Get color based on scheme and intensity
fn scheme_color(scheme: u8, intensity: u8, bold: bool) -> (Color, bool) {
    match scheme {
        1 => match intensity {  // Red/Yellow (fire)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::DarkYellow, bold),
            _ => (Color::Yellow, true),
        },
        2 => match intensity {  // Blue/Cyan (ice)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::DarkCyan, bold),
            _ => (Color::Cyan, true),
        },
        3 => match intensity {  // Magenta/Red (pink)
            0 => (Color::DarkMagenta, false),
            1 => (Color::Magenta, false),
            2 => (Color::Red, bold),
            _ => (Color::White, true),
        },
        4 => match intensity {  // Yellow/White (gold)
            0 => (Color::DarkYellow, false),
            1 => (Color::Yellow, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        5 => match intensity {  // Cyan/White (electric)
            0 => (Color::DarkCyan, false),
            1 => (Color::Cyan, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        6 => match intensity {  // Red/Magenta (lava)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::Magenta, bold),
            _ => (Color::White, true),
        },
        7 => match intensity {  // White/Grey (mono)
            0 => (Color::DarkGrey, false),
            1 => (Color::Grey, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        8 => match intensity {  // Rainbow cycling
            0 => (Color::Red, false),
            1 => (Color::Yellow, false),
            2 => (Color::Green, bold),
            _ => (Color::Cyan, true),
        },
        9 => match intensity {  // Blue/Magenta (neon)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::Magenta, bold),
            _ => (Color::White, true),
        },
        _ => match intensity {  // Default: Green (matrix) - classic cmatrix look
            0 => (Color::DarkGreen, false),
            1 => (Color::Green, false),
            2 => (Color::Green, true),
            _ => (Color::Rgb { r: 180, g: 255, b: 180 }, true),  // Bright green-white for head
        },
    }
}

/// Run the visualization
pub fn run(config: FractalConfig) -> io::Result<()> {
    let seed = config.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    let mut rng = StdRng::seed_from_u64(seed);
    let mut term = Terminal::new(true)?;

    term.clear_screen()?;

    match config.fractal_type {
        crate::config::FractalType::Matrix => run_matrix(&mut term, &config, &mut rng),
        crate::config::FractalType::Life => run_life(&mut term, &config, &mut rng),
        crate::config::FractalType::Plasma => run_plasma(&mut term, &config),
        crate::config::FractalType::Fire => run_fire(&mut term, &config, &mut rng),
        crate::config::FractalType::Rain => run_rain(&mut term, &config, &mut rng),
        crate::config::FractalType::Waves => run_waves(&mut term, &config),
        crate::config::FractalType::Cube => run_cube(&mut term, &config),
        crate::config::FractalType::Pipes => run_pipes(&mut term, &config, &mut rng),
        crate::config::FractalType::Donut => run_donut(&mut term, &config),
    }
}

/// Matrix rain effect (cmatrix-like)
fn run_matrix(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    struct Drop {
        y: f32,
        speed: f32,
        length: usize,
        chars: Vec<char>,
    }

    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789@#$%^&*(){}[]|;:,.<>?~`"
        .chars().collect();

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut drops: Vec<Drop> = (0..w)
        .map(|_| {
            Drop {
                y: rng.gen_range(-(h as f32)..0.0),
                speed: rng.gen_range(0.5..1.2),
                length: rng.gen_range(5..20),
                chars: (0..25).map(|_| chars[rng.gen_range(0..chars.len())]).collect(),
            }
        })
        .collect();

    loop {
        // Check for terminal resize
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            // Resize drops vector
            drops.resize_with(w, || {
                Drop {
                    y: rng.gen_range(-(h as f32)..0.0),
                    speed: rng.gen_range(0.5..1.2),
                    length: rng.gen_range(5..20),
                    chars: (0..25).map(|_| chars[rng.gen_range(0..chars.len())]).collect(),
                }
            });
        }

        // Handle input
        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Clear back buffer
        term.clear();

        // Populate back buffer with all drops
        for x in 0..w {
            let drop = &drops[x];
            let head_y = drop.y as i32;
            let len = drop.length;
            let half_len = len / 2;

            for i in 0..len {
                let y = head_y - i as i32;
                if y >= 0 && y < h as i32 {
                    let char_idx = (y as usize + x) % drop.chars.len();
                    let ch = drop.chars[char_idx];

                    // Color gradient: head -> near head -> middle -> tail
                    let intensity = if i == 0 { 3 } else if i < 3 { 2 } else if i < half_len { 1 } else { 0 };
                    let (color, bold) = scheme_color(state.color_scheme, intensity, i < 3);

                    term.set(x as i32, y, ch, Some(color), bold);
                }
            }
        }

        // Present with differential update (only changed cells)
        term.present()?;

        // Update drop positions
        for x in 0..w {
            let drop = &mut drops[x];
            drop.y += drop.speed;

            // Reset drop when it goes off screen
            let head_y = drop.y as i32;
            if head_y - drop.length as i32 >= h as i32 {
                drop.y = rng.gen_range(-20.0..0.0);
                drop.speed = rng.gen_range(0.5..1.2);
                drop.length = rng.gen_range(5..20);
                for c in &mut drop.chars {
                    if rng.gen_bool(0.3) {
                        *c = chars[rng.gen_range(0..chars.len())];
                    }
                }
            }
        }

        term.sleep(state.speed);
    }

    Ok(())
}

/// Conway's Game of Life
fn run_life(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut grid: Vec<Vec<bool>> = (0..h)
        .map(|_| (0..w).map(|_| rng.gen_bool(0.3)).collect())
        .collect();

    let mut next_grid = grid.clone();
    let mut neighbor_counts: Vec<Vec<u8>> = vec![vec![0; w]; h];
    let mut generation = 0u64;

    loop {
        // Check for terminal resize
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            grid = (0..h)
                .map(|_| (0..w).map(|_| rng.gen_bool(0.3)).collect())
                .collect();
            next_grid = grid.clone();
            neighbor_counts = vec![vec![0; w]; h];
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Compute neighbor counts
        for y in 0..h {
            for x in 0..w {
                neighbor_counts[y][x] = count_neighbors(&grid, x, y, w, h);
            }
        }

        // Clear and populate back buffer
        term.clear();
        for y in 0..h {
            for x in 0..w {
                let neighbors = neighbor_counts[y][x];
                let alive = grid[y][x];

                if alive {
                    let intensity = match neighbors { 2 => 1, 3 => 2, _ => 0 };
                    let (color, bold) = scheme_color(state.color_scheme, intensity, false);
                    term.set(x as i32, y as i32, config.draw_char, Some(color), bold);
                }

                next_grid[y][x] = match (alive, neighbors) {
                    (true, 2) | (true, 3) => true,
                    (false, 3) => true,
                    _ => false,
                };
            }
        }

        term.present()?;
        term.sleep(state.speed);

        std::mem::swap(&mut grid, &mut next_grid);
        generation += 1;

        if generation % 100 == 0 {
            for _ in 0..((w * h) / 50) {
                let x = rng.gen_range(0..w);
                let y = rng.gen_range(0..h);
                grid[y][x] = true;
            }
        }
    }

    Ok(())
}

fn count_neighbors(grid: &[Vec<bool>], x: usize, y: usize, w: usize, h: usize) -> u8 {
    let mut count = 0;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
            let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
            if grid[ny][nx] {
                count += 1;
            }
        }
    }
    count
}

/// Plasma effect (animated sine waves)
fn run_plasma(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    let mut time: f64 = 0.0;
    let chars = [' ', '.', ':', ';', 'o', 'O', '0', '@', '#'];

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    const SIN_TABLE_SIZE: usize = 1024;
    let sin_table: Vec<f64> = (0..SIN_TABLE_SIZE)
        .map(|i| ((i as f64 / SIN_TABLE_SIZE as f64) * std::f64::consts::TAU).sin())
        .collect();

    let fast_sin = |x: f64| -> f64 {
        let normalized = x.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
        let idx = (normalized * SIN_TABLE_SIZE as f64) as usize;
        sin_table[idx.min(SIN_TABLE_SIZE - 1)]
    };

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        let w = width as f64;
        let h = height as f64;
        let inv_w = 1.0 / w;
        let inv_h = 1.0 / h;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        let time_1 = time;
        let time_1_5 = time * 1.5;
        let time_0_5 = time * 0.5;

        for y in 0..height {
            let fy = y as f64 * inv_h;
            let v2_base = fy * 10.0 + time_1_5;

            for x in 0..width {
                let fx = x as f64 * inv_w;

                let v1 = fast_sin(fx * 10.0 + time_1);
                let v2 = fast_sin(v2_base);
                let v3 = fast_sin((fx + fy) * 5.0 + time_0_5);
                let dx = fx - 0.5;
                let dy = fy - 0.5;
                let dist_sq = dx * dx + dy * dy;
                let v4 = fast_sin(dist_sq.sqrt() * 10.0 - time_1);

                let value = (v1 + v2 + v3 + v4) * 0.25;
                let normalized = (value + 1.0) * 0.5;

                let char_idx = (normalized * (chars.len() - 1) as f64) as usize;
                let ch = chars[char_idx.min(chars.len() - 1)];

                let intensity = (normalized * 3.0) as u8;
                let (color, bold) = scheme_color(state.color_scheme, intensity, normalized > 0.7);

                term.set(x as i32, y as i32, ch, Some(color), bold);
            }
        }

        term.present()?;
        time += (state.speed / 0.03) as f64 * 0.06;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Fire effect (doom-style)
fn run_fire(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 1; // Default to fire colors

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut fire: Vec<Vec<u8>> = vec![vec![0; w]; h];
    let fire_chars = [' ', '.', ':', ';', '*', 'o', 'O', '#', '@', '%'];

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            fire = vec![vec![0; w]; h];
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Set bottom row to max heat
        for x in 0..w {
            fire[h - 1][x] = if rng.gen_bool(0.8) { 255 } else { rng.gen_range(200..255) };
        }

        // Propagate fire upward
        for y in 0..h - 1 {
            for x in 0..w {
                let below = fire[y + 1][x] as u16;
                let left = if x > 0 { fire[y + 1][x - 1] as u16 } else { below };
                let right = if x < w - 1 { fire[y + 1][x + 1] as u16 } else { below };

                let avg = (below + left + right) / 3;
                let decay = rng.gen_range(0..15) as u16;
                fire[y][x] = avg.saturating_sub(decay).min(255) as u8;
            }
        }

        // Draw to back buffer
        for y in 0..h {
            for x in 0..w {
                let heat = fire[y][x];
                let char_idx = (heat as usize * (fire_chars.len() - 1)) / 255;
                let ch = fire_chars[char_idx.min(fire_chars.len() - 1)];
                let intensity = (heat / 64).min(3) as u8;
                let (color, bold) = scheme_color(state.color_scheme, intensity, heat > 200);
                term.set(x as i32, y as i32, ch, Some(color), bold);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Rain effect (falling raindrops with splashes)
fn run_rain(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 2; // Default to blue/cyan

    struct Raindrop { x: usize, y: f32, speed: f32, char: char }
    struct Splash { x: usize, y: usize, age: u8 }

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut drops: Vec<Raindrop> = Vec::new();
    let mut splashes: Vec<Splash> = Vec::new();
    let mut screen: Vec<Vec<char>> = vec![vec![' '; w]; h];

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            screen = vec![vec![' '; w]; h];
            drops.clear();
            splashes.clear();
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        for row in &mut screen {
            for cell in row { *cell = ' '; }
        }

        if rng.gen_bool(0.4) {
            drops.push(Raindrop {
                x: rng.gen_range(0..w),
                y: 0.0,
                speed: rng.gen_range(0.5..2.0),
                char: if rng.gen_bool(0.7) { '|' } else { '/' },
            });
        }

        let mut new_drops = Vec::new();
        for mut drop in drops {
            drop.y += drop.speed;
            let y = drop.y as usize;
            if y >= h - 1 {
                splashes.push(Splash { x: drop.x, y: h - 1, age: 0 });
            } else {
                if y < h && drop.x < w { screen[y][drop.x] = drop.char; }
                new_drops.push(drop);
            }
        }
        drops = new_drops;

        let splash_chars = ['~', '.', ' '];
        let mut new_splashes = Vec::new();
        for mut splash in splashes {
            if (splash.age as usize) < splash_chars.len() && splash.y < h && splash.x < w {
                let ch = splash_chars[splash.age as usize];
                if splash.x > 0 { screen[splash.y][splash.x - 1] = ch; }
                screen[splash.y][splash.x] = ch;
                if splash.x < w - 1 { screen[splash.y][splash.x + 1] = ch; }
                splash.age += 1;
                if (splash.age as usize) < splash_chars.len() {
                    new_splashes.push(splash);
                }
            }
        }
        splashes = new_splashes;

        term.clear();
        for y in 0..h {
            for x in 0..w {
                let ch = screen[y][x];
                if ch != ' ' {
                    let intensity = match ch { '|' | '/' => 2, '~' => 1, _ => 0 };
                    let (color, bold) = scheme_color(state.color_scheme, intensity, ch == '|' || ch == '/');
                    term.set(x as i32, y as i32, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Waves effect (animated sine waves)
fn run_waves(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 2; // Default to blue/cyan

    let mut time: f64 = 0.0;
    let wave_chars = ['_', '.', '-', '~', '^', '"', '*'];

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    const SIN_TABLE_SIZE: usize = 1024;
    let sin_table: Vec<f64> = (0..SIN_TABLE_SIZE)
        .map(|i| ((i as f64 / SIN_TABLE_SIZE as f64) * std::f64::consts::TAU).sin())
        .collect();

    let fast_sin = |x: f64| -> f64 {
        let normalized = x.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
        let idx = (normalized * SIN_TABLE_SIZE as f64) as usize;
        sin_table[idx.min(SIN_TABLE_SIZE - 1)]
    };

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        let h = height as f64;
        let mid_y = h / 2.0;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        term.clear();

        // Draw multiple wave layers
        for layer in 0..5 {
            let layer_f = layer as f64;
            let amplitude = (h / 4.0) * (1.0 - layer_f * 0.15);
            let frequency = 0.05 + layer_f * 0.02;
            let speed = 1.0 + layer_f * 0.3;
            let phase = layer_f * 0.5;

            let intensity = match layer { 0 => 0, 1 => 1, 2 => 1, 3 => 2, _ => 3 };
            let (color, bold) = scheme_color(state.color_scheme, intensity, layer == 4);

            for x in 0..width as usize {
                let fx = x as f64;
                let wave_y = mid_y + amplitude * fast_sin(fx * frequency + time * speed + phase);
                let y = wave_y as i32;

                if y >= 0 && y < height as i32 {
                    let char_idx = (fast_sin(fx * 0.3 + time * 2.0).abs() * (wave_chars.len() - 1) as f64) as usize;
                    let ch = wave_chars[char_idx.min(wave_chars.len() - 1)];
                    term.set(x as i32, y, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        time += (state.speed / 0.03) as f64 * 0.03;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Convert hue (0-360) to ANSI color
fn hue_to_ansi(hue: f64) -> Color {
    let h = hue % 360.0;
    match h as u32 {
        0..=59 => Color::Red,
        60..=119 => Color::Yellow,
        120..=179 => Color::Green,
        180..=239 => Color::Cyan,
        240..=299 => Color::Blue,
        _ => Color::Magenta,
    }
}

/// 3D rotating cube effect using braille characters
fn run_cube(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    let mut time: f32 = 0.0;

    let vertices: [(f32, f32, f32); 8] = [
        (-1.0, -1.0, -1.0), ( 1.0, -1.0, -1.0), ( 1.0,  1.0, -1.0), (-1.0,  1.0, -1.0),
        (-1.0, -1.0,  1.0), ( 1.0, -1.0,  1.0), ( 1.0,  1.0,  1.0), (-1.0,  1.0,  1.0),
    ];

    let edges: [(usize, usize); 12] = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];

    let distance = 3.5;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut braille_w = init_w as usize * 2;
    let mut braille_h = init_h as usize * 4;
    let mut braille_dots: Vec<Vec<bool>> = vec![vec![false; braille_w]; braille_h];
    let mut projected: Vec<(f32, f32)> = vec![(0.0, 0.0); 8];

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            braille_w = width as usize * 2;
            braille_h = height as usize * 4;
            braille_dots = vec![vec![false; braille_w]; braille_h];
        }

        let w = width as f32;
        let h = height as f32;
        let half_w = w / 2.0;
        let half_h = h / 2.0;
        let cube_size = (h * 2.0).min(w) * 0.4;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        for row in &mut braille_dots {
            for cell in row { *cell = false; }
        }

        let rx = time * 0.6;
        let ry = time * 0.8;
        let rz = time * 0.4;

        let (cos_x, sin_x) = (rx.cos(), rx.sin());
        let (cos_y, sin_y) = (ry.cos(), ry.sin());
        let (cos_z, sin_z) = (rz.cos(), rz.sin());

        for (i, &(x, y, z)) in vertices.iter().enumerate() {
            let y1 = y * cos_x - z * sin_x;
            let z1 = y * sin_x + z * cos_x;
            let x2 = x * cos_y + z1 * sin_y;
            let z2 = -x * sin_y + z1 * cos_y;
            let x3 = x2 * cos_z - y1 * sin_z;
            let y3 = x2 * sin_z + y1 * cos_z;

            let z_factor = 1.0 / (distance + z2);
            let screen_x = half_w + x3 * z_factor * cube_size;
            let screen_y = half_h + y3 * z_factor * cube_size * 0.5;

            projected[i] = (screen_x, screen_y);
        }

        for &(v1, v2) in &edges {
            let (x0, y0) = projected[v1];
            let (x1, y1) = projected[v2];

            let bx0 = (x0 * 2.0) as i32;
            let by0 = (y0 * 4.0) as i32;
            let bx1 = (x1 * 2.0) as i32;
            let by1 = (y1 * 4.0) as i32;

            let dx = (bx1 - bx0).abs();
            let dy = -(by1 - by0).abs();
            let sx = if bx0 < bx1 { 1 } else { -1 };
            let sy = if by0 < by1 { 1 } else { -1 };
            let mut err = dx + dy;
            let mut x = bx0;
            let mut y = by0;

            loop {
                if x >= 0 && x < braille_w as i32 && y >= 0 && y < braille_h as i32 {
                    braille_dots[y as usize][x as usize] = true;
                }

                if x == bx1 && y == by1 { break; }

                let e2 = 2 * err;
                if e2 >= dy {
                    if x == bx1 { break; }
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    if y == by1 { break; }
                    err += dx;
                    y += sy;
                }
            }
        }

        term.clear();
        for cy in 0..height as usize {
            let by = cy * 4;
            for cx in 0..width as usize {
                let bx = cx * 2;

                let mut dots: u8 = 0;
                if braille_dots[by][bx] { dots |= 0x01; }
                if braille_dots[by + 1][bx] { dots |= 0x02; }
                if braille_dots[by + 2][bx] { dots |= 0x04; }
                if braille_dots[by][bx + 1] { dots |= 0x08; }
                if braille_dots[by + 1][bx + 1] { dots |= 0x10; }
                if braille_dots[by + 2][bx + 1] { dots |= 0x20; }
                if braille_dots[by + 3][bx] { dots |= 0x40; }
                if braille_dots[by + 3][bx + 1] { dots |= 0x80; }

                if dots > 0 {
                    let ch = char::from_u32(0x2800 + dots as u32).unwrap_or(' ');
                    let (color, bold) = scheme_color(state.color_scheme, 2, true);
                    term.set(cx as i32, cy as i32, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        time += (state.speed / 0.03) * 0.06;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Classic pipes screensaver
fn run_pipes(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let pipe_chars: [[char; 4]; 4] = [
        ['│', '└', '│', '┘'],
        ['┘', '─', '┐', '─'],
        ['│', '┌', '│', '┐'],
        ['└', '─', '┌', '─'],
    ];

    let pipe_colors = [
        Color::Red, Color::Green, Color::Yellow, Color::Blue,
        Color::Magenta, Color::Cyan, Color::White,
    ];

    struct Pipe { x: i32, y: i32, dir: u8, color: Color, steps: u32 }

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut pipes: Vec<Pipe> = Vec::new();
    let mut fill_count: usize = 0;

    let spawn_pipe = |rng: &mut StdRng, w: usize, h: usize, pipe_colors: &[Color]| -> Pipe {
        let dir = rng.gen_range(0..4);
        let (x, y) = match dir {
            0 => (rng.gen_range(0..w as i32), h as i32 - 1),
            1 => (0, rng.gen_range(0..h as i32)),
            2 => (rng.gen_range(0..w as i32), 0),
            _ => (w as i32 - 1, rng.gen_range(0..h as i32)),
        };
        Pipe { x, y, dir, color: pipe_colors[rng.gen_range(0..pipe_colors.len())], steps: 0 }
    };

    for _ in 0..5 {
        pipes.push(spawn_pipe(rng, w, h, &pipe_colors));
    }

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            fill_count = 0;
            pipes.clear();
            for _ in 0..5 { pipes.push(spawn_pipe(rng, w, h, &pipe_colors)); }
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) { break; }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        if fill_count > (w * h * 7) / 10 {
            term.clear_screen()?;
            fill_count = 0;
            pipes.clear();
            for _ in 0..5 { pipes.push(spawn_pipe(rng, w, h, &pipe_colors)); }
        }

        for pipe in &mut pipes {
            let old_dir = pipe.dir;
            if pipe.steps > 3 && rng.gen_bool(0.25) {
                pipe.dir = if rng.gen_bool(0.5) { (pipe.dir + 1) % 4 } else { (pipe.dir + 3) % 4 };
                pipe.steps = 0;
            }

            let ch = pipe_chars[old_dir as usize][pipe.dir as usize];

            if pipe.x >= 0 && pipe.x < w as i32 && pipe.y >= 0 && pipe.y < h as i32 {
                fill_count += 1;
                let (color, bold) = scheme_color(state.color_scheme, 2, true);
                term.set(pipe.x, pipe.y, ch, Some(color), bold);
            }

            match pipe.dir {
                0 => pipe.y -= 1,
                1 => pipe.x += 1,
                2 => pipe.y += 1,
                _ => pipe.x -= 1,
            }
            pipe.steps += 1;

            if pipe.x < 0 || pipe.x >= w as i32 || pipe.y < 0 || pipe.y >= h as i32 {
                *pipe = spawn_pipe(rng, w, h, &pipe_colors);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Rotating 3D donut (torus) effect
fn run_donut(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    let mut a: f32 = 0.0;
    let mut b: f32 = 0.0;

    let luminance_chars: [char; 12] = ['.', ',', '-', '~', ':', ';', '=', '!', '*', '#', '$', '@'];

    let r1 = 1.0;
    let r2 = 2.0;
    let k2 = 5.0;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut z_buffer: Vec<Vec<f32>> = vec![vec![0.0; init_w as usize]; init_h as usize];
    let mut output: Vec<Vec<char>> = vec![vec![' '; init_w as usize]; init_h as usize];
    let mut lum_buffer: Vec<Vec<f32>> = vec![vec![0.0; init_w as usize]; init_h as usize];

    let theta_step = 0.07;
    let phi_step = 0.02;
    let theta_count = (std::f32::consts::TAU / theta_step) as usize + 1;
    let phi_count = (std::f32::consts::TAU / phi_step) as usize + 1;

    let theta_trig: Vec<(f32, f32)> = (0..theta_count)
        .map(|i| { let theta = i as f32 * theta_step; (theta.cos(), theta.sin()) })
        .collect();

    let phi_trig: Vec<(f32, f32)> = (0..phi_count)
        .map(|i| { let phi = i as f32 * phi_step; (phi.cos(), phi.sin()) })
        .collect();

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            z_buffer = vec![vec![0.0; width as usize]; height as usize];
            output = vec![vec![' '; width as usize]; height as usize];
            lum_buffer = vec![vec![0.0; width as usize]; height as usize];
        }

        let w = width as f32;
        let h = height as f32;
        let k1 = h * k2 * 3.0 / (8.0 * (r1 + r2));
        let half_w = w / 2.0;
        let half_h = h / 2.0;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) { break; }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        for y in 0..height as usize {
            for x in 0..width as usize {
                z_buffer[y][x] = 0.0;
                output[y][x] = ' ';
                lum_buffer[y][x] = 0.0;
            }
        }

        let (cos_a, sin_a) = (a.cos(), a.sin());
        let (cos_b, sin_b) = (b.cos(), b.sin());
        let cos_a_sin_b = cos_a * sin_b;
        let sin_a_sin_b = sin_a * sin_b;
        let sin_a_cos_b = sin_a * cos_b;

        for (cos_theta, sin_theta) in &theta_trig {
            let circle_x = r2 + r1 * cos_theta;
            let circle_y = r1 * sin_theta;
            let circle_y_cos_a = circle_y * cos_a;
            let circle_y_sin_a = circle_y * sin_a;

            for (cos_phi, sin_phi) in &phi_trig {
                let x = circle_x * (cos_b * cos_phi + sin_a_sin_b * sin_phi) - circle_y * cos_a_sin_b;
                let y = circle_x * (sin_b * cos_phi - sin_a_cos_b * sin_phi) + circle_y_cos_a * cos_b;
                let z = k2 + cos_a * circle_x * sin_phi + circle_y_sin_a;
                let ooz = 1.0 / z;

                let xp = (half_w + k1 * ooz * x) as i32;
                let yp = (half_h - k1 * ooz * y * 0.5) as i32;

                if xp >= 0 && xp < width as i32 && yp >= 0 && yp < height as i32 {
                    let px = xp as usize;
                    let py = yp as usize;

                    if ooz > z_buffer[py][px] {
                        z_buffer[py][px] = ooz;

                        let l = cos_phi * cos_theta * sin_b - cos_a * cos_theta * sin_phi
                            - sin_a * sin_theta + cos_b * (cos_a * sin_theta - cos_theta * sin_a * sin_phi);
                        lum_buffer[py][px] = l;

                        let lum_idx = if l > 0.0 { ((l * 8.0) as usize).min(luminance_chars.len() - 1) } else { 0 };
                        output[py][px] = luminance_chars[lum_idx];
                    }
                }
            }
        }

        term.clear();
        for y in 0..height as usize {
            for x in 0..width as usize {
                let ch = output[y][x];
                if ch != ' ' {
                    let l = lum_buffer[y][x];
                    let intensity = if l > 0.6 { 3 } else if l > 0.3 { 2 } else if l > 0.0 { 1 } else { 0 };
                    let (color, bold) = scheme_color(state.color_scheme, intensity, l > 0.5);
                    term.set(x as i32, y as i32, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        a += 0.04 * (state.speed / 0.03);
        b += 0.02 * (state.speed / 0.03);
        term.sleep(state.speed);
    }

    Ok(())
}
