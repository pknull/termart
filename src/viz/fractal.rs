//! Fractal visualization with braille rendering
//!
//! Multiple fractal types with manual zoom/pan and animated Julia morphing.
//!
//! Controls:
//! - F: Cycle fractal type
//! - P: Cycle Julia paths (Julia mode only)
//! - +/-: Zoom in/out
//! - Arrows: Pan view
//! - R: Reset view
//! - 1-9: Animation speed (N × 10 seconds per cycle)
//! - Shift+1-9: Color scheme
//! - Space: Pause
//! - Q/Esc: Quit

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::event::KeyCode;
use std::io;

// Braille constants (2x4 dot grid per character)
const BRAILLE_BASE: u32 = 0x2800;
const DOTS_X: usize = 2;
const DOTS_Y: usize = 4;

// Iteration limits
const MAX_ITER: u32 = 80;

/// Fractal type
#[derive(Clone, Copy, PartialEq)]
enum FractalType {
    Julia,
    Mandelbrot,
    BurningShip,
    Tricorn,
    Phoenix,
}

impl FractalType {
    fn next(self) -> Self {
        match self {
            Self::Julia => Self::Mandelbrot,
            Self::Mandelbrot => Self::BurningShip,
            Self::BurningShip => Self::Tricorn,
            Self::Tricorn => Self::Phoenix,
            Self::Phoenix => Self::Julia,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Julia => "Julia",
            Self::Mandelbrot => "Mandelbrot",
            Self::BurningShip => "Burning Ship",
            Self::Tricorn => "Tricorn",
            Self::Phoenix => "Phoenix",
        }
    }
}

// Julia constant paths - (center_x, center_y, radius, name)
// The constant animates in a circle around the center point
const JULIA_PATHS: &[(f64, f64, f64, &str)] = &[
    (-0.75, 0.0, 0.15, "Main Cardioid Edge"),
    (-0.1, 0.75, 0.08, "Upper Filament"),
    (0.28, 0.0, 0.02, "Elephant Valley"),
    (-0.12, 0.0, 0.75, "Wide Sweep"),
    (-0.4, 0.6, 0.05, "Rabbit Ears"),
];

struct FractalState {
    // Fractal type
    fractal_type: FractalType,
    // Julia constant (animated)
    cx: f64,
    cy: f64,
    // Animation angle
    angle: f64,
    // Current path
    path_idx: usize,
    // Zoom level (1.0 = default view, higher = zoomed in)
    zoom: f64,
    // Pan offset
    pan_x: f64,
    pan_y: f64,
    // Phoenix fractal: previous z value per pixel (stored separately)
    phoenix_p: f64,
    // Animation speed level (1-9), each level = 10 seconds per cycle
    speed_level: u8,
}

impl FractalState {
    fn new() -> Self {
        let (px, py, r, _) = JULIA_PATHS[0];
        Self {
            fractal_type: FractalType::Julia,
            cx: px + r,
            cy: py,
            angle: 0.0,
            path_idx: 0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            phoenix_p: -0.5,  // Phoenix parameter
            speed_level: 5,   // Default: 50 seconds per cycle
        }
    }

    fn set_speed(&mut self, level: u8) {
        self.speed_level = level.clamp(1, 9);
    }

    fn cycle_type(&mut self) {
        self.fractal_type = self.fractal_type.next();
        // Reset view when changing type
        self.reset_view();
    }

    fn cycle_path(&mut self) {
        self.path_idx = (self.path_idx + 1) % JULIA_PATHS.len();
        self.angle = 0.0;
    }

    fn zoom_in(&mut self) {
        self.zoom *= 1.2;
    }

    fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.1);
    }

    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    fn pan(&mut self, dx: f64, dy: f64) {
        // Pan speed inversely proportional to zoom
        let pan_speed = 0.1 / self.zoom;
        self.pan_x += dx * pan_speed;
        self.pan_y += dy * pan_speed;
    }

    fn update(&mut self, dt: f64) {
        // Only animate for Julia mode
        if self.fractal_type != FractalType::Julia {
            return;
        }

        // Animate the Julia constant along a circular path
        self.angle += dt;  // dt already scaled by caller
        if self.angle > std::f64::consts::TAU {
            self.angle -= std::f64::consts::TAU;
        }

        let (px, py, r, _) = JULIA_PATHS[self.path_idx];
        self.cx = px + r * self.angle.cos();
        self.cy = py + r * self.angle.sin();
    }

    fn path_name(&self) -> &str {
        JULIA_PATHS[self.path_idx].3
    }
}

/// Compute Julia iteration count: z = z² + c, where c is constant
#[inline]
fn julia_iter(mut zx: f64, mut zy: f64, cx: f64, cy: f64) -> u32 {
    let mut iter = 0u32;
    while zx * zx + zy * zy <= 4.0 && iter < MAX_ITER {
        let xtemp = zx * zx - zy * zy + cx;
        zy = 2.0 * zx * zy + cy;
        zx = xtemp;
        iter += 1;
    }
    iter
}

/// Compute Mandelbrot iteration count: z = z² + c, where c is the pixel
#[inline]
fn mandelbrot_iter(cx: f64, cy: f64) -> u32 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut iter = 0u32;
    while zx * zx + zy * zy <= 4.0 && iter < MAX_ITER {
        let xtemp = zx * zx - zy * zy + cx;
        zy = 2.0 * zx * zy + cy;
        zx = xtemp;
        iter += 1;
    }
    iter
}

/// Compute Burning Ship iteration: z = (|Re(z)| + i|Im(z)|)² + c
#[inline]
fn burning_ship_iter(cx: f64, cy: f64) -> u32 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut iter = 0u32;
    while zx * zx + zy * zy <= 4.0 && iter < MAX_ITER {
        let xtemp = zx * zx - zy * zy + cx;
        zy = 2.0 * zx.abs() * zy.abs() + cy;
        zx = xtemp;
        iter += 1;
    }
    iter
}

/// Compute Tricorn (Mandelbar) iteration: z = conj(z)² + c
#[inline]
fn tricorn_iter(cx: f64, cy: f64) -> u32 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut iter = 0u32;
    while zx * zx + zy * zy <= 4.0 && iter < MAX_ITER {
        let xtemp = zx * zx - zy * zy + cx;
        zy = -2.0 * zx * zy + cy;  // Note the negative for conjugate
        zx = xtemp;
        iter += 1;
    }
    iter
}

/// Compute Phoenix iteration: z = z² + Re(c) + p*z_prev, Im(c) added to imaginary
#[inline]
fn phoenix_iter(cx: f64, cy: f64, p: f64) -> u32 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    let mut prev_zx = 0.0;
    let mut prev_zy = 0.0;
    let mut iter = 0u32;
    while zx * zx + zy * zy <= 4.0 && iter < MAX_ITER {
        let xtemp = zx * zx - zy * zy + cx + p * prev_zx;
        let ytemp = 2.0 * zx * zy + cy + p * prev_zy;
        prev_zx = zx;
        prev_zy = zy;
        zx = xtemp;
        zy = ytemp;
        iter += 1;
    }
    iter
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

/// Help text
const HELP: &str = "\
FRACTALS
─────────────────
F      Cycle type
P      Cycle path
+/-    Zoom in/out
Arrows Pan view
R      Reset view
1-9    Speed (N×10s)";

/// Run the fractal visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step, HELP);
    let mut fractal = FractalState::new();

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Iteration buffer (stores iteration count per braille dot)
    let mut iter_grid: Vec<Vec<u32>> = Vec::new();

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
        }

        // Initialize/resize grid
        let grid_w = width as usize * DOTS_X;
        let grid_h = height as usize * DOTS_Y;
        if iter_grid.len() != grid_h || iter_grid.first().map_or(0, |r| r.len()) != grid_w {
            iter_grid = vec![vec![0; grid_w]; grid_h];
        }

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
            match code {
                KeyCode::Char('f') | KeyCode::Char('F') => fractal.cycle_type(),
                KeyCode::Char('p') | KeyCode::Char('P') => fractal.cycle_path(),
                KeyCode::Char('+') | KeyCode::Char('=') => fractal.zoom_in(),
                KeyCode::Char('-') | KeyCode::Char('_') => fractal.zoom_out(),
                KeyCode::Char('r') | KeyCode::Char('R') => fractal.reset_view(),
                KeyCode::Up => fractal.pan(0.0, -1.0),
                KeyCode::Down => fractal.pan(0.0, 1.0),
                KeyCode::Left => fractal.pan(-1.0, 0.0),
                KeyCode::Right => fractal.pan(1.0, 0.0),
                // Speed: 1-9 = 10-90 seconds per cycle
                KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                    fractal.set_speed(c.to_digit(10).unwrap() as u8);
                }
                _ => {}
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

        // Compute fractal - viewport with zoom and pan
        let aspect = (grid_w as f64) / (grid_h as f64) * 2.0;
        let base_scale = 3.5;  // Base scale showing full fractal
        let scale = base_scale / fractal.zoom;  // Higher zoom = smaller scale

        for gy in 0..grid_h {
            for gx in 0..grid_w {
                // Map grid position to complex plane with pan offset
                let x = (gx as f64 / grid_w as f64 - 0.5) * scale * aspect + fractal.pan_x;
                let y = (gy as f64 / grid_h as f64 - 0.5) * scale + fractal.pan_y;

                iter_grid[gy][gx] = match fractal.fractal_type {
                    FractalType::Julia => julia_iter(x, y, fractal.cx, fractal.cy),
                    FractalType::Mandelbrot => mandelbrot_iter(x, y),
                    FractalType::BurningShip => burning_ship_iter(x, y),
                    FractalType::Tricorn => tricorn_iter(x, y),
                    FractalType::Phoenix => phoenix_iter(x, y, fractal.phoenix_p),
                };
            }
        }

        // Render to terminal
        term.clear();

        for cy in 0..height as usize {
            for cx in 0..width as usize {
                let mut dots = [[false; DOTS_X]; DOTS_Y];
                let mut max_iter = 0u32;

                // Sample 2x4 braille dots
                for dy in 0..DOTS_Y {
                    for dx in 0..DOTS_X {
                        let gx = cx * DOTS_X + dx;
                        let gy = cy * DOTS_Y + dy;
                        if gx < grid_w && gy < grid_h {
                            let iter = iter_grid[gy][gx];
                            // Point is "inside" if it reached max iterations
                            dots[dy][dx] = iter < MAX_ITER;
                            max_iter = max_iter.max(iter);
                        }
                    }
                }

                let ch = encode_braille(&dots);

                // Color based on iteration count
                let intensity = if max_iter >= MAX_ITER {
                    0  // Inside the set
                } else {
                    ((max_iter as f32 / MAX_ITER as f32) * 3.0).min(3.0) as u8
                };

                let (color, bold) = scheme_color(state.color_scheme(), intensity, intensity >= 2);
                term.set(cx as i32, cy as i32, ch, Some(color), bold);
            }
        }

        // Show fractal name (and path for Julia)
        let name = if fractal.fractal_type == FractalType::Julia {
            format!(" {} - {} ", fractal.fractal_type.name(), fractal.path_name())
        } else {
            format!(" {} ", fractal.fractal_type.name())
        };
        let name_x = (width as usize).saturating_sub(name.len()) / 2;
        for (i, c) in name.chars().enumerate() {
            let (color, _) = scheme_color(state.color_scheme(), 3, true);
            term.set((name_x + i) as i32, 0, c, Some(color), true);
        }

        state.render_help(term, width, height);
        term.present()?;

        // Fixed frame rate, speed level determines cycle time
        // Speed N = N * 10 seconds per full cycle
        const FRAME_TIME: f32 = 0.033;  // ~30 FPS
        let cycle_seconds = fractal.speed_level as f64 * 10.0;
        let anim_speed = std::f64::consts::TAU / (cycle_seconds * 30.0);  // radians per frame
        fractal.update(anim_speed);

        term.sleep(FRAME_TIME);
    }

    Ok(())
}
