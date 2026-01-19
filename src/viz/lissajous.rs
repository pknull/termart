//! Lissajous curve visualization with braille rendering and trailing persistence
//!
//! Parametric curves: x = A*sin(a*t + δ), y = B*sin(b*t)
//! Multiple harmonics displayed simultaneously with phase animation.
//!
//! Controls:
//! - 1-9: Speed
//! - Shift+1-9: Color scheme
//! - Space: Pause
//! - H: Cycle through harmonic presets
//! - Q/Esc: Quit

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::event::KeyCode;
use crossterm::style::Color;
use std::io;

// Braille constants (2x4 dot grid per character)
const BRAILLE_BASE: u32 = 0x2800;
const DOTS_X: usize = 2;
const DOTS_Y: usize = 4;

// Trail persistence
const TRAIL_LENGTH: usize = 256;
const DECAY_RATE: f32 = 0.97;

// Harmonic presets: (a, b, name)
const HARMONICS: &[(f32, f32, &str)] = &[
    (3.0, 2.0, "3:2"),
    (5.0, 4.0, "5:4"),
    (3.0, 4.0, "3:4"),
    (5.0, 6.0, "5:6"),
    (7.0, 6.0, "7:6"),
    (9.0, 8.0, "9:8"),
    (1.0, 2.0, "1:2"),
    (2.0, 3.0, "2:3"),
    (4.0, 5.0, "4:5"),
];

#[derive(Clone, Copy)]
struct TrailPoint {
    x: f32,
    y: f32,
    age: f32, // 1.0 = new, decays toward 0
}

struct LissajousCurve {
    a: f32,           // x frequency
    b: f32,           // y frequency
    delta: f32,       // phase shift (animated)
    trail: Vec<TrailPoint>,
}

impl LissajousCurve {
    fn new(a: f32, b: f32) -> Self {
        Self {
            a,
            b,
            delta: 0.0,
            trail: Vec::with_capacity(TRAIL_LENGTH),
        }
    }

    fn update(&mut self, dt: f32) {
        // Animate phase shift
        self.delta += dt * 0.5;
        if self.delta > std::f32::consts::TAU {
            self.delta -= std::f32::consts::TAU;
        }

        // Generate new points along the curve
        let steps = 8; // Points per frame
        for i in 0..steps {
            let t_offset = (i as f32 / steps as f32) * std::f32::consts::TAU / 60.0;
            let t = self.delta * 10.0 + t_offset;

            let x = (self.a * t + self.delta).sin();
            let y = (self.b * t).sin();

            self.trail.push(TrailPoint { x, y, age: 1.0 });
        }

        // Age existing points
        for point in &mut self.trail {
            point.age *= DECAY_RATE;
        }

        // Remove dead points
        self.trail.retain(|p| p.age > 0.02);

        // Cap trail length
        while self.trail.len() > TRAIL_LENGTH {
            self.trail.remove(0);
        }
    }
}

/// Encode 2x4 dot pattern to braille character
fn encode_braille(dots: &[[bool; DOTS_X]; DOTS_Y]) -> char {
    // Braille dot positions:
    // 0 3
    // 1 4
    // 2 5
    // 6 7
    let mut code: u32 = 0;
    if dots[0][0] { code |= 1 << 0; }
    if dots[1][0] { code |= 1 << 1; }
    if dots[2][0] { code |= 1 << 2; }
    if dots[0][1] { code |= 1 << 3; }
    if dots[1][1] { code |= 1 << 4; }
    if dots[2][1] { code |= 1 << 5; }
    if dots[3][0] { code |= 1 << 6; }
    if dots[3][1] { code |= 1 << 7; }

    char::from_u32(BRAILLE_BASE + code).unwrap_or(' ')
}

/// Run the Lissajous visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut harmonic_idx: usize = 0;
    let (a, b, _) = HARMONICS[harmonic_idx];
    let mut curve = LissajousCurve::new(a, b);

    // Intensity grid for braille rendering (stores max intensity per dot)
    let mut intensity_grid: Vec<Vec<f32>> = Vec::new();

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            // Resize intensity grid
            let grid_w = width as usize * DOTS_X;
            let grid_h = height as usize * DOTS_Y;
            intensity_grid = vec![vec![0.0; grid_w]; grid_h];
        }

        // Initialize grid if needed
        if intensity_grid.is_empty() {
            let grid_w = width as usize * DOTS_X;
            let grid_h = height as usize * DOTS_Y;
            intensity_grid = vec![vec![0.0; grid_w]; grid_h];
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
            // Handle 'H' for harmonic cycling
            if let KeyCode::Char('h') | KeyCode::Char('H') = code {
                harmonic_idx = (harmonic_idx + 1) % HARMONICS.len();
                let (a, b, _) = HARMONICS[harmonic_idx];
                curve = LissajousCurve::new(a, b);
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Guard against zero-size terminal
        if width == 0 || height == 0 {
            term.sleep(0.1);
            continue;
        }

        // Update curve
        curve.update(state.speed);

        // Clear intensity grid (decay existing values)
        for row in &mut intensity_grid {
            for val in row {
                *val *= 0.85; // Faster decay for crisp trails
            }
        }

        // Plot trail points onto intensity grid
        let grid_w = width as usize * DOTS_X;
        let grid_h = height as usize * DOTS_Y;
        let cx = grid_w as f32 / 2.0;
        let cy = grid_h as f32 / 2.0;
        let scale = (cx.min(cy) * 0.85).max(1.0);

        for point in &curve.trail {
            let px = (cx + point.x * scale) as usize;
            let py = (cy + point.y * scale) as usize;

            if px < grid_w && py < grid_h {
                // Accumulate intensity (newer points brighter)
                intensity_grid[py][px] = intensity_grid[py][px].max(point.age);
            }
        }

        // Render braille characters
        term.clear();

        for char_y in 0..height as usize {
            for char_x in 0..width as usize {
                let mut dots = [[false; DOTS_X]; DOTS_Y];
                let mut max_intensity: f32 = 0.0;

                for dy in 0..DOTS_Y {
                    for dx in 0..DOTS_X {
                        let gx = char_x * DOTS_X + dx;
                        let gy = char_y * DOTS_Y + dy;

                        if gy < grid_h && gx < grid_w {
                            let intensity = intensity_grid[gy][gx];
                            if intensity > 0.1 {
                                dots[dy][dx] = true;
                                max_intensity = max_intensity.max(intensity);
                            }
                        }
                    }
                }

                if max_intensity > 0.1 {
                    let ch = encode_braille(&dots);
                    let level = ((max_intensity * 4.0) as u8).min(3);
                    let (color, bold) = scheme_color(state.color_scheme(), level, max_intensity > 0.7);
                    term.set(char_x as i32, char_y as i32, ch, Some(color), bold);
                }
            }
        }

        // Draw harmonic ratio indicator
        let (_, _, name) = HARMONICS[harmonic_idx];
        let label = format!("Lissajous {} [H:cycle]", name);
        term.set_str(1, 0, &label, Some(Color::DarkGrey), false);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
