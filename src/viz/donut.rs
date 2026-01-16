//! Rotating 3D donut (torus) effect visualization

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use std::io;

// Torus geometry constants
const TORUS_INNER_RADIUS: f32 = 1.0;
const TORUS_TUBE_RADIUS: f32 = 2.0;
const VIEWER_DISTANCE: f32 = 5.0;
const THETA_STEP: f32 = 0.07;
const PHI_STEP: f32 = 0.02;
const MIN_Z_DIVISOR: f32 = 0.01;

/// Run the rotating donut visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    let mut a: f32 = 0.0;
    let mut b: f32 = 0.0;

    let luminance_chars: [char; 12] = ['.', ',', '-', '~', ':', ';', '=', '!', '*', '#', '$', '@'];

    let r1 = TORUS_INNER_RADIUS;
    let r2 = TORUS_TUBE_RADIUS;
    let k2 = VIEWER_DISTANCE;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    let mut z_buffer: Vec<Vec<f32>> = vec![vec![0.0; init_w as usize]; init_h as usize];
    let mut output: Vec<Vec<char>> = vec![vec![' '; init_w as usize]; init_h as usize];
    let mut lum_buffer: Vec<Vec<f32>> = vec![vec![0.0; init_w as usize]; init_h as usize];

    let theta_step = THETA_STEP;
    let phi_step = PHI_STEP;
    let theta_count = (std::f32::consts::TAU / theta_step) as usize + 1;
    let phi_count = (std::f32::consts::TAU / phi_step) as usize + 1;

    let theta_trig: Vec<(f32, f32)> = (0..theta_count)
        .map(|i| {
            let theta = i as f32 * theta_step;
            (theta.cos(), theta.sin())
        })
        .collect();

    let phi_trig: Vec<(f32, f32)> = (0..phi_count)
        .map(|i| {
            let phi = i as f32 * phi_step;
            (phi.cos(), phi.sin())
        })
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
            if state.handle_key(code, mods) {
                break;
            }
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
                let ooz = 1.0 / z.max(MIN_Z_DIVISOR);

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

                        let lum_idx = if l > 0.0 {
                            ((l * 11.0) as usize).min(luminance_chars.len() - 1)
                        } else {
                            0
                        };
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
                    let intensity = if l > 0.6 {
                        3
                    } else if l > 0.3 {
                        2
                    } else if l > 0.0 {
                        1
                    } else {
                        0
                    };
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
