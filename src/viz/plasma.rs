//! Plasma effect visualization (animated sine waves)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

/// Run the plasma effect visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
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
