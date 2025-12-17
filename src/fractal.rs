use crate::config::FractalConfig;
use crate::terminal::Terminal;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Color;
use rand::prelude::*;
use std::io;

// Speed lookup table for number keys (0-9)
const SPEED_TABLE: [f32; 10] = [0.2, 0.005, 0.01, 0.02, 0.03, 0.05, 0.07, 0.1, 0.15, 0.2];

/// Runtime state for interactive controls
struct VizState {
    speed: f32,        // Current speed (time per frame)
    color_scheme: u8,  // Current color scheme (0-9)
    paused: bool,
}

impl VizState {
    #[inline]
    fn new(initial_speed: f32) -> Self {
        Self {
            speed: initial_speed,
            color_scheme: 0,
            paused: false,
        }
    }

    /// Handle keypress, returns true if should quit
    #[inline]
    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char(' ') => self.paused = !self.paused,
            // Number keys: change speed (1=fastest, 9=slowest, 0=very slow)
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.speed = SPEED_TABLE[(c as u8 - b'0') as usize];
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
#[inline]
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
            _ => (Color::White, true),  // Bright white for head
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
        crate::config::FractalType::Matrix => crate::viz::matrix::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Life => crate::viz::life::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Plasma => crate::viz::plasma::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Fire => crate::viz::fire::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Rain => crate::viz::rain::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Waves => crate::viz::waves::run(&mut term, &config),
        crate::config::FractalType::Cube => crate::viz::cube::run(&mut term, &config),
        crate::config::FractalType::Pipes => crate::viz::pipes::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Donut => crate::viz::donut::run(&mut term, &config),
        crate::config::FractalType::Globe => crate::viz::globe::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Hex => crate::viz::hex::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Keyboard => crate::viz::keyboard::run(&mut term, &config),
        crate::config::FractalType::Invaders => crate::viz::invaders::run(&mut term, &config, &mut rng),
    }
}

/// Conway's Game of Life
pub fn run_life(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
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
        for (y, row) in neighbor_counts.iter_mut().enumerate() {
            for (x, count) in row.iter_mut().enumerate() {
                *count = count_neighbors(&grid, x, y, w, h);
            }
        }

        // Clear and populate back buffer
        term.clear();
        for (y, (grid_row, count_row)) in grid.iter().zip(neighbor_counts.iter()).enumerate() {
            for (x, (&alive, &neighbors)) in grid_row.iter().zip(count_row.iter()).enumerate() {

                if alive {
                    let intensity = match neighbors { 2 => 1, 3 => 2, _ => 0 };
                    let (color, bold) = scheme_color(state.color_scheme, intensity, false);
                    term.set(x as i32, y as i32, config.draw_char, Some(color), bold);
                }

                next_grid[y][x] = matches!((alive, neighbors), (true, 2) | (true, 3) | (false, 3));
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

#[inline]
fn count_neighbors(grid: &[Vec<bool>], x: usize, y: usize, w: usize, h: usize) -> u8 {
    let mut count = 0u8;
    let xi = x as i32;
    let yi = y as i32;
    let wi = w as i32;
    let hi = h as i32;
    // Unrolled loop for all 8 neighbors
    let neighbors = [
        ((xi - 1).rem_euclid(wi) as usize, (yi - 1).rem_euclid(hi) as usize),
        (x, (yi - 1).rem_euclid(hi) as usize),
        ((xi + 1).rem_euclid(wi) as usize, (yi - 1).rem_euclid(hi) as usize),
        ((xi - 1).rem_euclid(wi) as usize, y),
        ((xi + 1).rem_euclid(wi) as usize, y),
        ((xi - 1).rem_euclid(wi) as usize, (yi + 1).rem_euclid(hi) as usize),
        (x, (yi + 1).rem_euclid(hi) as usize),
        ((xi + 1).rem_euclid(wi) as usize, (yi + 1).rem_euclid(hi) as usize),
    ];
    for (nx, ny) in neighbors {
        if grid[ny][nx] { count += 1; }
    }
    count
}

/// Plasma effect (animated sine waves)
pub fn run_plasma(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    use rand::Rng;

    let mut state = VizState::new(config.time_step);
    let mut time: f64 = 0.0;
    let chars = [' ', '.', ':', ';', 'o', 'O', '0', '@', '#'];

    // Seed-dependent parameters for unique patterns
    let freq1: f64 = rng.gen_range(6.0..14.0);      // X frequency
    let freq2: f64 = rng.gen_range(6.0..14.0);      // Y frequency
    let freq3: f64 = rng.gen_range(3.0..8.0);       // Diagonal frequency
    let freq4: f64 = rng.gen_range(6.0..14.0);      // Radial frequency
    let phase1: f64 = rng.gen_range(0.0..6.28);     // Phase offsets
    let phase2: f64 = rng.gen_range(0.0..6.28);
    let phase3: f64 = rng.gen_range(0.0..6.28);
    let phase4: f64 = rng.gen_range(0.0..6.28);
    let center_x: f64 = rng.gen_range(0.3..0.7);    // Radial center
    let center_y: f64 = rng.gen_range(0.3..0.7);
    let time_mult1: f64 = rng.gen_range(0.7..1.3);  // Time multipliers
    let time_mult2: f64 = rng.gen_range(1.2..1.8);
    let time_mult3: f64 = rng.gen_range(0.3..0.7);

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

        let t1 = time * time_mult1;
        let t2 = time * time_mult2;
        let t3 = time * time_mult3;

        for y in 0..height {
            let fy = y as f64 * inv_h;
            let v2_base = fy * freq2 + phase2 + t2;

            for x in 0..width {
                let fx = x as f64 * inv_w;

                let v1 = fast_sin(fx * freq1 + phase1 + t1);
                let v2 = fast_sin(v2_base);
                let v3 = fast_sin((fx + fy) * freq3 + phase3 + t3);
                let dx = fx - center_x;
                let dy = fy - center_y;
                let dist_sq = dx * dx + dy * dy;
                let v4 = fast_sin(dist_sq.sqrt() * freq4 + phase4 - t1);

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
pub fn run_fire(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
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
        for (y, row) in fire.iter().enumerate() {
            for (x, &heat) in row.iter().enumerate() {
                let char_idx = (heat as usize * (fire_chars.len() - 1)) / 255;
                let ch = fire_chars[char_idx.min(fire_chars.len() - 1)];
                let intensity = (heat / 64).min(3);
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
pub fn run_rain(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
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
        for (y, row) in screen.iter().enumerate() {
            for (x, &ch) in row.iter().enumerate() {
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
pub fn run_waves(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
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

/// 3D rotating cube effect using braille characters
pub fn run_cube(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
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
pub fn run_pipes(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let pipe_chars: [[char; 4]; 4] = [
        ['│', '└', '│', '┘'],
        ['┘', '─', '┐', '─'],
        ['│', '┌', '│', '┐'],
        ['└', '─', '┌', '─'],
    ];

    struct Pipe { x: i32, y: i32, dir: u8, steps: u32 }

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut pipes: Vec<Pipe> = Vec::new();
    let mut fill_count: usize = 0;

    let spawn_pipe = |rng: &mut StdRng, w: usize, h: usize| -> Pipe {
        let dir = rng.gen_range(0..4);
        let (x, y) = match dir {
            0 => (rng.gen_range(0..w as i32), h as i32 - 1),
            1 => (0, rng.gen_range(0..h as i32)),
            2 => (rng.gen_range(0..w as i32), 0),
            _ => (w as i32 - 1, rng.gen_range(0..h as i32)),
        };
        Pipe { x, y, dir, steps: 0 }
    };

    for _ in 0..5 {
        pipes.push(spawn_pipe(rng, w, h));
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
            for _ in 0..5 { pipes.push(spawn_pipe(rng, w, h)); }
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
            for _ in 0..5 { pipes.push(spawn_pipe(rng, w, h)); }
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
                *pipe = spawn_pipe(rng, w, h);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Rotating 3D donut (torus) effect
pub fn run_donut(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
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

/// Fetch user's location from IP geolocation service
fn fetch_user_location() -> Option<(f32, f32)> {
    // Try ip-api.com (free, no key required)
    let resp = ureq::get("http://ip-api.com/json/?fields=lat,lon")
        .timeout(std::time::Duration::from_secs(3))
        .call()
        .ok()?;

    let body = resp.into_string().ok()?;

    // Simple JSON parsing without serde
    let lat = body.split("\"lat\":").nth(1)?
        .split(&[',', '}'][..]).next()?
        .trim().parse::<f32>().ok()?;
    let lon = body.split("\"lon\":").nth(1)?
        .split(&[',', '}'][..]).next()?
        .trim().parse::<f32>().ok()?;

    Some((lat.to_radians(), lon.to_radians()))
}

/// Rotating 3D globe with network activity (eDEX-UI style)
pub fn run_globe(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Default to electric (cyan/white)

    let mut rotation: f32 = 0.0;
    let tilt: f32 = 0.15; // Slight tilt for better equatorial view

    // Fetch user's location (non-blocking, falls back to None)
    let user_location: Option<(f32, f32)> = fetch_user_location();

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut braille_w = init_w as usize * 2;
    let mut braille_h = init_h as usize * 4;
    let mut braille_dots: Vec<Vec<u8>> = vec![vec![0; braille_w]; braille_h];

    // Network activity blips
    struct Blip {
        lat: f32,
        lon: f32,
        age: f32,
        max_age: f32,
    }

    // Connection arcs between points
    struct Arc {
        lat1: f32, lon1: f32,
        lat2: f32, lon2: f32,
        progress: f32,
    }

    let mut blips: Vec<Blip> = Vec::new();
    let mut arcs: Vec<Arc> = Vec::new();

    // Pulsing animation for user marker
    let mut user_pulse: f32 = 0.0;

    // Precompute trig tables
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

    // Continent outlines - coordinates in degrees, converted to radians
    // Format: (latitude, longitude) - lat: -90 to 90, lon: -180 to 180
    let deg_to_rad = |lat: f32, lon: f32| -> (f32, f32) {
        (lat.to_radians(), lon.to_radians())
    };

    // Natural Earth 110m simplified continent outlines (238 points total)
    let continents: Vec<Vec<(f32, f32)>> = vec![
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
    ];

    // Major world cities (lat, lon in radians) - for network activity blips
    let major_cities: Vec<(f32, f32)> = vec![
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
        deg_to_rad(-23.5, -46.6),  // São Paulo
        deg_to_rad(-22.9, -43.2),  // Rio de Janeiro
        deg_to_rad(-34.6, -58.4),  // Buenos Aires
        deg_to_rad(-33.4, -70.6),  // Santiago
        deg_to_rad(-12.0, -77.0),  // Lima
        deg_to_rad(4.7, -74.1),    // Bogotá
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
    ];

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
        let radius = (h * 1.8).min(w * 0.8) * 0.4;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Clear braille buffer (0 = empty, 1 = grid, 2 = continent, 3 = blip)
        for row in &mut braille_dots {
            for cell in row {
                *cell = 0;
            }
        }

        let (cos_tilt, sin_tilt) = (fast_cos(tilt), fast_sin(tilt));

        // Helper: convert lat/lon to screen coords
        let lat_lon_to_screen = |lat: f32, lon: f32| -> Option<(i32, i32, f32)> {
            // Sphere coordinates (standard spherical to cartesian)
            // x = right, y = into screen, z = up
            let cos_lat = fast_cos(lat);
            let sin_lat = fast_sin(lat);
            let cos_lon = fast_cos(lon + rotation); // Apply rotation to longitude
            let sin_lon = fast_sin(lon + rotation);

            let x = cos_lat * sin_lon;  // right/left
            let y = cos_lat * cos_lon;  // depth (into screen)
            let z = sin_lat;            // up/down

            // Apply tilt around X axis (tips the globe forward/back)
            let y2 = y * cos_tilt - z * sin_tilt;
            let z2 = y * sin_tilt + z * cos_tilt;

            // Only draw front-facing points (y2 > 0 means facing viewer)
            if y2 < -0.1 {
                return None;
            }

            let screen_x = half_w + x * radius;
            let screen_y = half_h - z2 * radius * 0.5; // Aspect correction

            let bx = (screen_x * 2.0) as i32;
            let by = (screen_y * 4.0) as i32;

            Some((bx, by, y2))
        };

        // Draw latitude lines (every 30 degrees)
        for lat_deg in (-60..=60).step_by(30) {
            let lat = (lat_deg as f32).to_radians();
            for lon_deg in 0..360 {
                let lon = (lon_deg as f32).to_radians() - std::f32::consts::PI;
                if let Some((bx, by, _)) = lat_lon_to_screen(lat, lon) {
                    if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32
                        && braille_dots[by as usize][bx as usize] == 0 {
                        braille_dots[by as usize][bx as usize] = 1;
                    }
                }
            }
        }

        // Draw longitude lines (every 30 degrees)
        for lon_deg in (0..360).step_by(30) {
            let lon = (lon_deg as f32).to_radians() - std::f32::consts::PI;
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
        for continent in &continents {
            // Draw points along the continent outline
            for i in 0..continent.len() {
                let (lat1, lon1) = continent[i];
                let (lat2, lon2) = continent[(i + 1) % continent.len()];

                // Interpolate between points
                for t in 0..20 {
                    let frac = t as f32 / 20.0;
                    let lat = lat1 + (lat2 - lat1) * frac;
                    let lon = lon1 + (lon2 - lon1) * frac;

                    if let Some((bx, by, _)) = lat_lon_to_screen(lat, lon) {
                        if bx >= 0 && bx < braille_w as i32 && by >= 0 && by < braille_h as i32 {
                            braille_dots[by as usize][bx as usize] = 2;
                        }
                    }
                }
            }
        }

        // Spawn new blips at major cities
        if rng.gen_bool(0.15) {
            let city_idx = rng.gen_range(0..major_cities.len());
            let (lat, lon) = major_cities[city_idx];
            blips.push(Blip {
                lat,
                lon,
                age: 0.0,
                max_age: rng.gen_range(0.5..2.0),
            });
        }

        // Spawn connection arcs occasionally
        if rng.gen_bool(0.03) && blips.len() >= 2 {
            let i1 = rng.gen_range(0..blips.len());
            let i2 = rng.gen_range(0..blips.len());
            if i1 != i2 {
                arcs.push(Arc {
                    lat1: blips[i1].lat,
                    lon1: blips[i1].lon,
                    lat2: blips[i2].lat,
                    lon2: blips[i2].lon,
                    progress: 0.0,
                });
            }
        }

        // Draw and update blips
        let mut new_blips = Vec::new();
        for mut blip in blips {
            blip.age += state.speed * 2.0;
            if blip.age < blip.max_age {
                // Draw blip with pulsing size based on age
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
                // Draw arc as great circle approximation
                let steps = (arc.progress * 30.0) as i32;
                for t in 0..=steps {
                    let frac = t as f32 / 30.0;
                    let lat = arc.lat1 + (arc.lat2 - arc.lat1) * frac;
                    let lon = arc.lon1 + (arc.lon2 - arc.lon1) * frac;
                    // Add slight arc height
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

        // Draw user location marker (pulsing, always visible when on front side)
        if let Some((user_lat, user_lon)) = user_location {
            user_pulse += state.speed * 3.0;
            let pulse_size = ((user_pulse.sin() + 1.0) * 2.0 + 2.0) as i32; // 2-6 size

            if let Some((bx, by, _)) = lat_lon_to_screen(user_lat, user_lon) {
                // Draw a larger, distinct marker (intensity 4 = user)
                for dy in -pulse_size..=pulse_size {
                    for dx in -pulse_size..=pulse_size {
                        // Diamond shape
                        if dx.abs() + dy.abs() <= pulse_size {
                            let px = bx + dx;
                            let py = by + dy;
                            if px >= 0 && px < braille_w as i32 && py >= 0 && py < braille_h as i32 {
                                braille_dots[py as usize][px as usize] = 4; // User marker
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

                // Check each dot position and track max intensity
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
                    // User marker (4) gets special orange color, others use scheme
                    let (color, bold) = if max_intensity == 4 {
                        (Color::Yellow, true) // Yellow for user
                    } else {
                        let intensity = match max_intensity {
                            1 => 0, // Grid lines - dim
                            2 => 2, // Continents - bright
                            _ => 3, // Blips/arcs - brightest
                        };
                        scheme_color(state.color_scheme, intensity, max_intensity >= 3)
                    };
                    term.set(cx as i32, cy as i32, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        rotation += 0.02 * (state.speed / 0.03);
        term.sleep(state.speed);
    }

    Ok(())
}

/// Hexagon grid with wave/pulse animations (eDEX-UI style)
pub fn run_hex(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Default to electric (cyan/white)

    let mut time: f32 = 0.0;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Hex cell dimensions (in characters)
    // Using flat-top hexagons:  / \
    //                          |   |
    //                           \ /
    let hex_width: usize = 6;  // Width of one hex cell
    let hex_height: usize = 3; // Height of one hex cell

    // Pulse origins (random points that emit waves)
    struct Pulse {
        x: f32,
        y: f32,
        birth_time: f32,
        speed: f32,
        max_radius: f32,
    }

    let mut pulses: Vec<Pulse> = Vec::new();

    // Precompute sin table
    const SIN_TABLE_SIZE: usize = 1024;
    let sin_table: Vec<f32> = (0..SIN_TABLE_SIZE)
        .map(|i| ((i as f32 / SIN_TABLE_SIZE as f32) * std::f32::consts::TAU).sin())
        .collect();

    let fast_sin = |x: f32| -> f32 {
        let normalized = x.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let idx = (normalized * SIN_TABLE_SIZE as f32) as usize;
        sin_table[idx.min(SIN_TABLE_SIZE - 1)]
    };

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            pulses.clear();
        }

        let w = width as usize;
        let h = height as usize;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Spawn new pulses randomly
        if rng.gen_bool(0.08) {
            pulses.push(Pulse {
                x: rng.gen_range(0.0..w as f32),
                y: rng.gen_range(0.0..h as f32),
                birth_time: time,
                speed: rng.gen_range(15.0..35.0),
                max_radius: rng.gen_range(30.0..80.0),
            });
        }

        // Remove old pulses
        pulses.retain(|p| {
            let age = time - p.birth_time;
            age * p.speed < p.max_radius + 10.0
        });

        term.clear();

        // Calculate hex grid dimensions - extend beyond screen for edge hexes
        let cols = (w / hex_width) + 3;
        let rows = (h / hex_height) + 3;

        // Helper to safely set a character (clips to screen bounds)
        let set_if_visible = |term: &mut Terminal, x: i32, y: i32, ch: char, color: Color, bold: bool| {
            if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                term.set(x, y, ch, Some(color), bold);
            }
        };

        // Draw hexagon grid - start before screen to catch partial hexes
        for row in 0..rows {
            for col in 0..cols {
                // Offset every other row for honeycomb pattern
                let x_offset: i32 = if row % 2 == 1 { (hex_width / 2) as i32 } else { 0 };
                let cx = (col as i32 * hex_width as i32) + x_offset - (hex_width as i32);
                let cy = (row as i32 * hex_height as i32) - (hex_height as i32);

                // Calculate intensity from pulses and ambient waves
                let mut intensity: f32 = 0.0;

                // Use center for wave calculations (clamp to valid range for visual continuity)
                let wave_x = cx.max(0) as f32;
                let wave_y = cy.max(0) as f32;

                // Ambient wave pattern
                let wave1 = fast_sin((wave_x * 0.1) + time * 2.0) * 0.3;
                let wave2 = fast_sin((wave_y * 0.15) + time * 1.5) * 0.2;
                let wave3 = fast_sin(((wave_x + wave_y) * 0.08) - time * 1.2) * 0.2;
                intensity += (wave1 + wave2 + wave3 + 0.7).max(0.0);

                // Pulse contributions
                for pulse in &pulses {
                    let dx = cx as f32 - pulse.x;
                    let dy = (cy as f32 - pulse.y) * 2.0; // Adjust for terminal aspect
                    let dist = (dx * dx + dy * dy).sqrt();
                    let age = time - pulse.birth_time;
                    let ring_pos = age * pulse.speed;

                    // Ring wave effect
                    let ring_dist = (dist - ring_pos).abs();
                    if ring_dist < 5.0 && dist < pulse.max_radius {
                        let ring_intensity = (1.0 - ring_dist / 5.0) * (1.0 - dist / pulse.max_radius);
                        intensity += ring_intensity * 1.5;
                    }
                }

                intensity = intensity.clamp(0.0, 1.5);

                // Draw hex based on intensity
                if intensity > 0.1 {
                    let level = ((intensity * 4.0) as u8).min(3);
                    let (color, bold) = scheme_color(state.color_scheme, level, intensity > 0.8);

                    // Draw hexagon shape - let set_if_visible handle clipping
                    // Top:    /_\
                    // Mid:   |   |
                    // Bot:    \_/
                    let x = cx - 2;
                    let y = cy - 1;

                    if intensity > 0.3 {
                        // Full hex - no top underscore to avoid doubling
                        //  / \
                        // |   |
                        //  \_/
                        set_if_visible(term, x + 1, y, '/', color, bold);
                        set_if_visible(term, x + 3, y, '\\', color, bold);
                        set_if_visible(term, x, y + 1, '|', color, bold);
                        set_if_visible(term, x + 4, y + 1, '|', color, bold);
                        set_if_visible(term, x + 1, y + 2, '\\', color, bold);
                        set_if_visible(term, x + 2, y + 2, '_', color, bold);
                        set_if_visible(term, x + 3, y + 2, '/', color, bold);
                    } else {
                        // Dim hex - just corners
                        set_if_visible(term, x + 1, y, '.', color, bold);
                        set_if_visible(term, x + 3, y, '.', color, bold);
                        set_if_visible(term, x + 1, y + 2, '.', color, bold);
                        set_if_visible(term, x + 3, y + 2, '.', color, bold);
                    }
                }
            }
        }

        term.present()?;
        time += state.speed * 2.0;
        term.sleep(state.speed);
    }

    Ok(())
}

/// Find keyboard input devices via evdev
fn find_keyboard_devices() -> Vec<evdev::Device> {
    let mut keyboards = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("event") {
                    if let Ok(device) = evdev::Device::open(&path) {
                        // Check if device has key events (is a keyboard)
                        if device.supported_keys().is_some_and(|keys| {
                            keys.contains(evdev::Key::KEY_A) && keys.contains(evdev::Key::KEY_SPACE)
                        }) {
                            keyboards.push(device);
                        }
                    }
                }
            }
        }
    }
    keyboards
}

/// Map evdev key code to display label
fn evdev_key_to_label(key: evdev::Key) -> Option<&'static str> {
    use evdev::Key;
    Some(match key {
        Key::KEY_ESC => "Esc",
        Key::KEY_1 => "1", Key::KEY_2 => "2", Key::KEY_3 => "3", Key::KEY_4 => "4", Key::KEY_5 => "5",
        Key::KEY_6 => "6", Key::KEY_7 => "7", Key::KEY_8 => "8", Key::KEY_9 => "9", Key::KEY_0 => "0",
        Key::KEY_MINUS => "-", Key::KEY_EQUAL => "=", Key::KEY_BACKSPACE => "Bksp",
        Key::KEY_TAB => "Tab",
        Key::KEY_Q => "Q", Key::KEY_W => "W", Key::KEY_E => "E", Key::KEY_R => "R", Key::KEY_T => "T",
        Key::KEY_Y => "Y", Key::KEY_U => "U", Key::KEY_I => "I", Key::KEY_O => "O", Key::KEY_P => "P",
        Key::KEY_LEFTBRACE => "[", Key::KEY_RIGHTBRACE => "]", Key::KEY_BACKSLASH => "\\",
        Key::KEY_CAPSLOCK => "Caps",
        Key::KEY_A => "A", Key::KEY_S => "S", Key::KEY_D => "D", Key::KEY_F => "F", Key::KEY_G => "G",
        Key::KEY_H => "H", Key::KEY_J => "J", Key::KEY_K => "K", Key::KEY_L => "L",
        Key::KEY_SEMICOLON => ";", Key::KEY_APOSTROPHE => "'", Key::KEY_ENTER => "Enter",
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => "Shift",
        Key::KEY_Z => "Z", Key::KEY_X => "X", Key::KEY_C => "C", Key::KEY_V => "V", Key::KEY_B => "B",
        Key::KEY_N => "N", Key::KEY_M => "M",
        Key::KEY_COMMA => ",", Key::KEY_DOT => ".", Key::KEY_SLASH => "/",
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => "Ctrl",
        Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => "Meta",
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => "Alt",
        Key::KEY_SPACE => "Space",
        Key::KEY_GRAVE => "`",
        Key::KEY_F1 => "F1", Key::KEY_F2 => "F2", Key::KEY_F3 => "F3", Key::KEY_F4 => "F4",
        Key::KEY_F5 => "F5", Key::KEY_F6 => "F6", Key::KEY_F7 => "F7", Key::KEY_F8 => "F8",
        Key::KEY_F9 => "F9", Key::KEY_F10 => "F10", Key::KEY_F11 => "F11", Key::KEY_F12 => "F12",
        Key::KEY_COMPOSE => "Menu",
        _ => return None,
    })
}

/// On-screen keyboard visualization with global key monitoring via evdev
pub fn run_keyboard(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Default to electric (cyan/white)

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Key press tracking with fade-out
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};

    let key_heat: Arc<Mutex<HashMap<String, f32>>> = Arc::new(Mutex::new(HashMap::new()));
    let shift_held = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));

    // Find keyboard devices
    let keyboards = find_keyboard_devices();
    let has_evdev = !keyboards.is_empty();

    // Spawn evdev listener threads for each keyboard
    let mut handles = Vec::new();
    for mut device in keyboards {
        let heat_clone = Arc::clone(&key_heat);
        let shift_clone = Arc::clone(&shift_held);
        let running_clone = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            // Get raw fd and set non-blocking via nix
            use std::os::unix::io::AsRawFd;
            let fd = device.as_raw_fd();
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }

            while running_clone.load(Ordering::Relaxed) {
                if let Ok(events) = device.fetch_events() {
                    for ev in events {
                        if let evdev::InputEventKind::Key(key) = ev.kind() {
                            // Track shift state
                            if matches!(key, evdev::Key::KEY_LEFTSHIFT | evdev::Key::KEY_RIGHTSHIFT) {
                                shift_clone.store(ev.value() != 0, Ordering::Relaxed);
                            }
                            // Value 1 = press, 0 = release, 2 = repeat
                            if ev.value() == 1 || ev.value() == 2 {
                                if let Some(label) = evdev_key_to_label(key) {
                                    if let Ok(mut heat) = heat_clone.lock() {
                                        heat.insert(label.to_string(), 1.0);
                                    }
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    // Keyboard layout (US QWERTY) - (normal_label, shifted_label, width)
    // F-keys row only shown in debug mode
    let f_row: Vec<(&str, &str, f32)> = vec![
        ("Esc", "Esc", 1.0), ("", "", 0.5), ("F1", "F1", 1.0), ("F2", "F2", 1.0), ("F3", "F3", 1.0), ("F4", "F4", 1.0),
        ("", "", 0.25), ("F5", "F5", 1.0), ("F6", "F6", 1.0), ("F7", "F7", 1.0), ("F8", "F8", 1.0),
        ("", "", 0.25), ("F9", "F9", 1.0), ("F10", "F10", 1.0), ("F11", "F11", 1.0), ("F12", "F12", 1.0),
    ];

    let mut rows: Vec<Vec<(&str, &str, f32)>> = Vec::new();
    if config.debug {
        rows.push(f_row);
    }
    // Row 1: Numbers
    rows.push(vec![
        ("`", "~", 1.0), ("1", "!", 1.0), ("2", "@", 1.0), ("3", "#", 1.0), ("4", "$", 1.0), ("5", "%", 1.0),
        ("6", "^", 1.0), ("7", "&", 1.0), ("8", "*", 1.0), ("9", "(", 1.0), ("0", ")", 1.0),
        ("-", "_", 1.0), ("=", "+", 1.0), ("Bksp", "Bksp", 1.5),
    ]);
    // Row 2: QWERTY top
    rows.push(vec![
        ("Tab", "Tab", 1.5), ("q", "Q", 1.0), ("w", "W", 1.0), ("e", "E", 1.0), ("r", "R", 1.0), ("t", "T", 1.0),
        ("y", "Y", 1.0), ("u", "U", 1.0), ("i", "I", 1.0), ("o", "O", 1.0), ("p", "P", 1.0),
        ("[", "{", 1.0), ("]", "}", 1.0), ("\\", "|", 1.5),
    ]);
    // Row 3: Home row
    rows.push(vec![
        ("Caps", "Caps", 1.75), ("a", "A", 1.0), ("s", "S", 1.0), ("d", "D", 1.0), ("f", "F", 1.0), ("g", "G", 1.0),
        ("h", "H", 1.0), ("j", "J", 1.0), ("k", "K", 1.0), ("l", "L", 1.0), (";", ":", 1.0),
        ("'", "\"", 1.0), ("Enter", "Enter", 2.25),
    ]);
    // Row 4: Shift row
    rows.push(vec![
        ("Shift", "Shift", 2.25), ("z", "Z", 1.0), ("x", "X", 1.0), ("c", "C", 1.0), ("v", "V", 1.0), ("b", "B", 1.0),
        ("n", "N", 1.0), ("m", "M", 1.0), (",", "<", 1.0), (".", ">", 1.0), ("/", "?", 1.0), ("Shift", "Shift", 2.75),
    ]);
    // Row 5: Bottom row (Meta key displays as "M")
    rows.push(vec![
        ("Ctrl", "Ctrl", 1.5), ("Meta", "Meta", 1.0), ("Alt", "Alt", 1.25), ("Space", "Space", 6.25),
        ("Alt", "Alt", 1.25), ("Meta", "Meta", 1.0), ("Menu", "Menu", 1.0), ("Ctrl", "Ctrl", 1.5),
    ]);

    // Key dimensions (compact mode)
    let key_width: f32 = 3.0;
    let key_height: usize = 1;

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        let w = width as usize;
        let h = height as usize;

        // Handle input (color scheme changes, speed, quit)
        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        // Decay heat values
        if let Ok(mut heat) = key_heat.lock() {
            for v in heat.values_mut() {
                *v = (*v - state.speed * 3.0).max(0.0);
            }
            heat.retain(|_, v| *v > 0.0);
        }

        term.clear();

        // Calculate keyboard vertical position (centered)
        let total_height = rows.len() * key_height + rows.len();
        let start_y = ((h - total_height) / 2).max(1);

        // Draw keyboard
        let heat_snapshot: HashMap<String, f32> = key_heat.lock().map(|h| h.clone()).unwrap_or_default();
        let is_shifted = shift_held.load(Ordering::Relaxed);

        for (row_idx, row) in rows.iter().enumerate() {
            let y = start_y + row_idx * (key_height + 1);

            // Calculate this row's total width for centering
            let row_key_count = row.iter().filter(|(l, _, _)| !l.is_empty()).count();
            let row_width_units: f32 = row.iter().map(|(_, _, w)| w).sum();
            let row_total_width = (row_width_units * key_width) as usize + row_key_count.saturating_sub(1);
            let mut x = ((w.saturating_sub(row_total_width)) / 2).max(1);

            for (normal_label, shifted_label, width_mult) in row {
                if normal_label.is_empty() {
                    x += (key_width * width_mult) as usize;
                    continue;
                }

                let key_w = (key_width * width_mult) as usize;
                // Heat lookup - evdev labels match exactly for special keys, uppercase for letters
                let heat_key = match *normal_label {
                    "Ctrl" | "Alt" | "Meta" | "Shift" | "Caps" | "Tab" | "Enter" | "Bksp" |
                    "Esc" | "Space" | "Menu" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" |
                    "F7" | "F8" | "F9" | "F10" | "F11" | "F12" => normal_label.to_string(),
                    _ => normal_label.to_uppercase(),
                };
                let heat = heat_snapshot.get(&heat_key).copied().unwrap_or(0.0);

                // Choose label based on shift state, with display overrides
                let base_label = if is_shifted { *shifted_label } else { *normal_label };
                let display_label = match base_label {
                    "Meta" => "M",  // Display Meta key as just "M"
                    other => other,
                };

                let (color, bold) = if heat > 0.7 {
                    (Color::White, true)
                } else if heat > 0.3 {
                    scheme_color(state.color_scheme, 3, true)
                } else if heat > 0.0 {
                    scheme_color(state.color_scheme, 2, false)
                } else {
                    scheme_color(state.color_scheme, 0, false)
                };

                // Draw compact key (label with padding, no brackets)
                if y < h {
                    // Center the label within the key width
                    let truncated: String = display_label.chars().take(key_w).collect();
                    let label_start = x + (key_w.saturating_sub(truncated.len())) / 2;
                    for (i, ch) in truncated.chars().enumerate() {
                        term.set((label_start + i) as i32, y as i32, ch, Some(color), bold);
                    }
                }

                x += key_w + 1;  // Add 1 char padding between keys
            }
        }

        // Debug status bar (only in debug mode)
        if config.debug {
            let status = if has_evdev { "[GLOBAL]" } else { "[LOCAL]" };
            let status_text = format!("{} (q to quit)", status);
            let status_x = ((w as f32 - status_text.len() as f32) / 2.0).max(0.0) as usize;
            let (status_color, _) = scheme_color(state.color_scheme, 1, false);
            for (i, ch) in status_text.chars().enumerate() {
                term.set((status_x + i) as i32, 0, ch, Some(status_color), false);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    // Signal threads to stop and wait for them
    running.store(false, Ordering::Relaxed);
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
