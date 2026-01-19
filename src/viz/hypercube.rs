//! N-dimensional rotating hypercube using braille characters.
//!
//! Renders a rotating hypercube (tesseract and beyond) using Unicode braille
//! characters for sub-cell resolution. Supports 1D through 16D with interactive
//! controls for dimension switching and zoom.
//!
//! # Controls
//! - Up/Down: Change dimension count (1D-16D)
//! - Left/Right or +/-: Zoom in/out
//! - Space: Pause
//! - Q/Esc: Quit

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::event::KeyCode;
use std::io;

/// Constants for hypercube visualization parameters.
mod constants {
    /// Default starting dimension (4D = tesseract)
    pub const DEFAULT_DIMENSIONS: usize = 4;
    /// Minimum supported dimension
    pub const MIN_DIMENSIONS: usize = 1;
    /// Maximum supported dimension (2^16 = 65536 vertices)
    pub const MAX_DIMENSIONS: usize = 16;

    /// Default zoom level
    pub const DEFAULT_ZOOM: f32 = 1.0;
    /// Minimum zoom level
    pub const MIN_ZOOM: f32 = 0.1;
    /// Maximum zoom level (prevents overflow)
    pub const MAX_ZOOM: f32 = 100.0;
    /// Zoom adjustment factor per keypress
    pub const ZOOM_FACTOR: f32 = 1.2;

    /// Perspective distance for higher dimension projection
    pub const PERSPECTIVE_DISTANCE: f32 = 4.0;
    /// Z-axis distance for 3D to 2D projection
    pub const Z_PROJECTION_DISTANCE: f32 = 3.5;
    /// Minimum perspective factor (prevents collapse)
    pub const PERSPECTIVE_FACTOR_MIN: f32 = 0.2;
    /// Maximum perspective factor (prevents explosion)
    pub const PERSPECTIVE_FACTOR_MAX: f32 = 2.0;

    /// Base rotation speed for XY plane
    pub const ROTATION_SPEED_XY: f32 = 0.3;
    /// Base rotation speed for YZ plane
    pub const ROTATION_SPEED_YZ: f32 = 0.25;
    /// Base rotation speed for XZ plane
    pub const ROTATION_SPEED_XZ: f32 = 0.15;
    /// Base rotation speed for higher dimensions
    pub const ROTATION_SPEED_BASE_HIGHER: f32 = 0.5;
    /// Speed reduction per higher dimension
    pub const ROTATION_SPEED_DECAY: f32 = 0.05;
    /// Minimum rotation speed for higher dimensions
    pub const ROTATION_SPEED_MIN: f32 = 0.1;
    /// Secondary rotation speed multiplier
    pub const ROTATION_SECONDARY_FACTOR: f32 = 0.6;

    /// Minimum extent value to avoid division by zero
    pub const MIN_EXTENT: f32 = 0.001;
    /// Scale factor for screen projection
    pub const SCREEN_SCALE: f32 = 0.45;
    /// Aspect ratio correction for terminal characters
    pub const ASPECT_CORRECTION: f32 = 0.5;

    /// Base Unicode code point for braille patterns (U+2800)
    pub const BRAILLE_BASE: u32 = 0x2800;
    /// Braille dots per character horizontally
    pub const BRAILLE_WIDTH: usize = 2;
    /// Braille dots per character vertically
    pub const BRAILLE_HEIGHT: usize = 4;

    /// Sleep time when paused
    pub const PAUSE_SLEEP: f32 = 0.1;
    /// Time step normalization factor
    pub const TIME_STEP_NORM: f32 = 0.03;
    /// Animation time increment
    pub const TIME_INCREMENT: f32 = 0.06;
    /// Time wrapping period to prevent precision loss (TAU * 1000 radians)
    pub const TIME_WRAP_PERIOD: f32 = std::f32::consts::TAU * 1000.0;
}

/// Generate all vertices for an n-dimensional hypercube (unit cube centered at origin).
///
/// Each vertex has coordinates of ±1 in each dimension. Uses bit manipulation:
/// for vertex index `i`, bit `d` determines whether dimension `d` is +1 or -1.
///
/// # Returns
/// A vector of 2^n vertices, each represented as an n-dimensional coordinate vector.
fn generate_vertices(dimensions: usize) -> Vec<Vec<f32>> {
    let count = 1 << dimensions; // 2^n vertices
    let mut vertices = Vec::with_capacity(count);

    for i in 0..count {
        let mut vertex = Vec::with_capacity(dimensions);
        for d in 0..dimensions {
            // Use bit d of i to determine +1 or -1
            vertex.push(if (i >> d) & 1 == 1 { 1.0 } else { -1.0 });
        }
        vertices.push(vertex);
    }
    vertices
}

/// Generate all edges for an n-dimensional hypercube.
///
/// Edges connect vertices that differ in exactly one coordinate (Hamming distance of 1).
/// Uses XOR to find adjacent vertices: flipping bit `d` gives the neighbor along axis `d`.
///
/// # Returns
/// A vector of (v1, v2) index pairs where v1 < v2 (avoids duplicates).
/// Total edge count: n × 2^(n-1)
fn generate_edges(dimensions: usize) -> Vec<(usize, usize)> {
    let vertex_count = 1 << dimensions;
    let mut edges = Vec::new();

    for v1 in 0..vertex_count {
        for d in 0..dimensions {
            let v2 = v1 ^ (1 << d); // Flip bit d
            if v1 < v2 { // Avoid duplicate edges
                edges.push((v1, v2));
            }
        }
    }
    edges
}

/// Apply rotation in a 2D plane defined by axes `a` and `b`.
///
/// Rotates the coordinates in-place using the standard 2D rotation matrix:
/// ```text
/// [cos θ  -sin θ] [x_a]
/// [sin θ   cos θ] [x_b]
/// ```
///
/// This generalizes 2D rotation to any pair of axes in n-dimensional space,
/// enabling rotations in planes like XY, XW, YZ, etc.
fn rotate_plane(coords: &mut [f32], a: usize, b: usize, angle: f32) {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let xa = coords[a];
    let xb = coords[b];
    coords[a] = xa * cos_a - xb * sin_a;
    coords[b] = xa * sin_a + xb * cos_a;
}

/// Project from n dimensions down to 2D using perspective projection.
///
/// Progressively projects from the highest dimension down to 2D,
/// applying perspective at each step with clamped factors to prevent
/// visual instability at extreme values.
///
/// # Panics
/// Returns (0.0, 0.0) for empty coordinate vectors (defensive guard).
fn project_to_2d(coords: &[f32]) -> (f32, f32) {
    use constants::*;

    // Guard against empty coords (should never happen with MIN_DIMENSIONS=1)
    if coords.is_empty() {
        return (0.0, 0.0);
    }

    let mut result = coords.to_vec();

    // Project from highest dimension down to 3D
    // Use weaker perspective for higher dimensions to maintain stability
    for d in (3..result.len()).rev() {
        let raw_factor = 1.0 / (PERSPECTIVE_DISTANCE - result[d]);
        // Clamp factor to prevent explosion/collapse
        let factor = raw_factor.clamp(PERSPECTIVE_FACTOR_MIN, PERSPECTIVE_FACTOR_MAX);
        for i in 0..d {
            result[i] *= factor;
        }
    }

    // Handle different dimensions
    match result.len() {
        1 => (result[0], 0.0), // 1D: just x
        2 => (result[0], result[1]), // 2D: x and y, no perspective
        _ => {
            // 3D+: perspective projection
            let raw_factor = 1.0 / (Z_PROJECTION_DISTANCE - result[2]);
            let factor = raw_factor.clamp(PERSPECTIVE_FACTOR_MIN, PERSPECTIVE_FACTOR_MAX);
            (result[0] * factor, result[1] * factor)
        }
    }
}

/// Encode an 8-dot braille pattern from a 2x4 grid of boolean dots.
///
/// Braille patterns use Unicode range U+2800-U+28FF where each bit
/// represents a dot position:
/// ```text
/// [0] [3]    bits: 0x01 0x08
/// [1] [4]          0x02 0x10
/// [2] [5]          0x04 0x20
/// [6] [7]          0x40 0x80
/// ```
fn encode_braille(dots: &[Vec<bool>], base_y: usize, base_x: usize, max_y: usize, max_x: usize) -> u8 {
    // Lookup table: [row][col] -> bit pattern
    // Row 0-2 are sequential bits, row 3 jumps to bits 6,7
    const BITS: [[u8; 2]; 4] = [
        [0x01, 0x08], // row 0: bits 0, 3
        [0x02, 0x10], // row 1: bits 1, 4
        [0x04, 0x20], // row 2: bits 2, 5
        [0x40, 0x80], // row 3: bits 6, 7
    ];

    let mut pattern: u8 = 0;
    for (row, bits) in BITS.iter().enumerate() {
        let y = base_y + row;
        if y < max_y {
            if base_x < max_x && dots[y][base_x] {
                pattern |= bits[0];
            }
            if base_x + 1 < max_x && dots[y][base_x + 1] {
                pattern |= bits[1];
            }
        }
    }
    pattern
}

/// Draw a line using Bresenham's algorithm into a dot buffer.
///
/// Sets all dots along the line from (x0, y0) to (x1, y1) to true,
/// with bounds checking against the buffer dimensions.
fn draw_line(dots: &mut [Vec<bool>], x0: i32, y0: i32, x1: i32, y1: i32, width: usize, height: usize) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            dots[y as usize][x as usize] = true;
        }

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
}

/// Run the n-dimensional rotating hypercube visualization.
///
/// Renders a rotating N-dimensional hypercube using braille characters
/// for sub-cell resolution. Supports interactive dimension switching (1D-16D)
/// and zoom controls.
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    use constants::*;

    let mut state = VizState::new(config.time_step);
    let mut time: f32 = 0.0;

    let mut dimensions: usize = DEFAULT_DIMENSIONS;
    let mut zoom: f32 = DEFAULT_ZOOM;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut braille_w = init_w as usize * BRAILLE_WIDTH;
    let mut braille_h = init_h as usize * BRAILLE_HEIGHT;
    let mut braille_dots: Vec<Vec<bool>> = vec![vec![false; braille_w]; braille_h];

    // Cache geometry - only regenerate when dimensions change
    let mut cached_dimensions = dimensions;
    let mut vertices = generate_vertices(dimensions);
    let mut edges = generate_edges(dimensions);

    // Reusable coordinate buffer to avoid allocations in hot loop
    let mut coord_buffer: Vec<f32> = vec![0.0; MAX_DIMENSIONS];

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            braille_w = width as usize * BRAILLE_WIDTH;
            braille_h = height as usize * BRAILLE_HEIGHT;
            braille_dots = vec![vec![false; braille_w]; braille_h];
        }

        let w = width as f32;
        let h = height as f32;
        let half_w = w / 2.0;
        let half_h = h / 2.0;
        let base_size = (h * 2.0).min(w);

        if let Some((code, mods)) = term.check_key()? {
            match code {
                KeyCode::Up => {
                    if dimensions < MAX_DIMENSIONS {
                        dimensions += 1;
                        term.clear_screen()?;
                    }
                }
                KeyCode::Down => {
                    if dimensions > MIN_DIMENSIONS {
                        dimensions -= 1;
                        term.clear_screen()?;
                    }
                }
                KeyCode::Right | KeyCode::Char('+') | KeyCode::Char('=') => {
                    zoom = (zoom * ZOOM_FACTOR).min(MAX_ZOOM);
                }
                KeyCode::Left | KeyCode::Char('-') | KeyCode::Char('_') => {
                    zoom = (zoom / ZOOM_FACTOR).max(MIN_ZOOM);
                }
                _ => {
                    if state.handle_key(code, mods) {
                        break;
                    }
                }
            }
        }

        if state.paused {
            term.sleep(PAUSE_SLEEP);
            continue;
        }

        // Clear braille buffer
        for row in &mut braille_dots {
            row.fill(false);
        }

        // Regenerate geometry only when dimensions change
        if dimensions != cached_dimensions {
            vertices = generate_vertices(dimensions);
            edges = generate_edges(dimensions);
            cached_dimensions = dimensions;
        }

        // Project all vertices (first pass: get raw projected coordinates)
        let mut raw_projected: Vec<(f32, f32)> = Vec::with_capacity(vertices.len());

        for vertex in &vertices {
            // Reuse coordinate buffer instead of cloning vertex
            coord_buffer.clear();
            coord_buffer.extend_from_slice(vertex);

            // Apply rotations in various planes dynamically
            // Base rotations in lower dimensions
            if dimensions >= 2 {
                rotate_plane(&mut coord_buffer, 0, 1, time * ROTATION_SPEED_XY);
            }
            if dimensions >= 3 {
                rotate_plane(&mut coord_buffer, 1, 2, time * ROTATION_SPEED_YZ);
                rotate_plane(&mut coord_buffer, 0, 2, time * ROTATION_SPEED_XZ);
            }
            // Higher dimension rotations: rotate axis 0 with each higher axis
            // This creates the "inside-out" effect for each dimension
            for d in 3..dimensions {
                let speed = ROTATION_SPEED_BASE_HIGHER - (d as f32 - 3.0) * ROTATION_SPEED_DECAY;
                rotate_plane(&mut coord_buffer, 0, d, time * speed.max(ROTATION_SPEED_MIN));
                // Also rotate alternating axes for visual interest
                let alt_axis = (d - 1) % 3; // Cycle through 0, 1, 2
                rotate_plane(&mut coord_buffer, alt_axis, d, time * (speed * ROTATION_SECONDARY_FACTOR));
            }

            let (px, py) = project_to_2d(&coord_buffer);
            raw_projected.push((px, py));
        }

        // Find max extent for normalization (so all dimensions appear same size)
        let max_extent = raw_projected.iter()
            .map(|(x, y)| x.abs().max(y.abs()))
            .fold(0.0f32, |a, b| a.max(b))
            .max(MIN_EXTENT);

        // Normalize and scale to screen coordinates (apply zoom)
        let projected: Vec<(f32, f32)> = raw_projected.iter()
            .map(|(px, py)| {
                let norm_x = px / max_extent;
                let norm_y = py / max_extent;
                let screen_x = half_w + norm_x * base_size * SCREEN_SCALE * zoom;
                let screen_y = half_h + norm_y * base_size * SCREEN_SCALE * ASPECT_CORRECTION * zoom;
                (screen_x, screen_y)
            })
            .collect();

        // Draw edges using Bresenham's line algorithm
        for &(v1, v2) in &edges {
            let (x0, y0) = projected[v1];
            let (x1, y1) = projected[v2];

            let bx0 = (x0 * BRAILLE_WIDTH as f32) as i32;
            let by0 = (y0 * BRAILLE_HEIGHT as f32) as i32;
            let bx1 = (x1 * BRAILLE_WIDTH as f32) as i32;
            let by1 = (y1 * BRAILLE_HEIGHT as f32) as i32;

            draw_line(&mut braille_dots, bx0, by0, bx1, by1, braille_w, braille_h);
        }

        term.clear();

        // Render braille characters
        for cy in 0..height as usize {
            let by = cy * BRAILLE_HEIGHT;
            for cx in 0..width as usize {
                let bx = cx * BRAILLE_WIDTH;

                let dots = encode_braille(&braille_dots, by, bx, braille_h, braille_w);

                if dots > 0 {
                    let ch = char::from_u32(BRAILLE_BASE + dots as u32).unwrap_or(' ');
                    let (color, bold) = scheme_color(state.color_scheme(), 2, true);
                    term.set(cx as i32, cy as i32, ch, Some(color), bold);
                }
            }
        }

        // Show dimension and zoom indicator
        let dim_text = format!("{}D", dimensions);
        let edge_count = edges.len();
        let vertex_count = vertices.len();
        let info = format!("{} ({}v/{}e) ↑↓dim ←→zoom:{:.1}x", dim_text, vertex_count, edge_count, zoom);
        for (i, ch) in info.chars().enumerate() {
            term.set(i as i32 + 1, 0, ch, None, false);
        }

        term.present()?;
        // Wrap time to prevent f32 precision loss after long runtime
        time = (time + (state.speed / TIME_STEP_NORM) * TIME_INCREMENT) % TIME_WRAP_PERIOD;
        term.sleep(state.speed);
    }

    Ok(())
}
