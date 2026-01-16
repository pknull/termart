//! 3D rotating cube effect using braille characters

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use std::io;

// Cube rendering constants
const CAMERA_DISTANCE: f32 = 3.5;
const CUBE_SCALE: f32 = 0.4;
const ROTATION_X_SPEED: f32 = 0.6;
const ROTATION_Y_SPEED: f32 = 0.8;
const ROTATION_Z_SPEED: f32 = 0.4;
const ASPECT_CORRECTION: f32 = 0.5;
const MIN_Z_DIVISOR: f32 = 0.1;

/// Run the 3D rotating cube visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
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
        let cube_size = (h * 2.0).min(w) * CUBE_SCALE;

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
            row.fill(false);
        }

        let rx = time * ROTATION_X_SPEED;
        let ry = time * ROTATION_Y_SPEED;
        let rz = time * ROTATION_Z_SPEED;

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

            let z_factor = 1.0 / (CAMERA_DISTANCE + z2).max(MIN_Z_DIVISOR);
            let screen_x = half_w + x3 * z_factor * cube_size;
            let screen_y = half_h + y3 * z_factor * cube_size * ASPECT_CORRECTION;

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

                if x == bx1 && y == by1 {
                    break;
                }

                let e2 = 2 * err;
                if e2 >= dy {
                    if x == bx1 {
                        break;
                    }
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    if y == by1 {
                        break;
                    }
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
        // Animation time advances at 2x frame rate
        time += state.speed * 2.0;
        term.sleep(state.speed);
    }

    Ok(())
}
