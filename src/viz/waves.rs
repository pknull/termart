//! Waves effect visualization (animated sine waves)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use std::io;

/// Run the waves effect visualization
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let mut time: f64 = 0.0;
    let wave_chars = ['_', '.', '-', '~', '^', '"', '*'];

    let (mut prev_w, mut prev_h) = term.size();

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

        // Wave layer rendering constants
        const NUM_LAYERS: usize = 5;
        const AMPLITUDE_DECAY: f64 = 0.15;
        const BASE_FREQUENCY: f64 = 0.05;
        const FREQUENCY_INCREMENT: f64 = 0.02;

        // Draw multiple wave layers
        for layer in 0..NUM_LAYERS {
            let layer_f = layer as f64;
            let amplitude = (h / 4.0) * (1.0 - layer_f * AMPLITUDE_DECAY);
            let frequency = BASE_FREQUENCY + layer_f * FREQUENCY_INCREMENT;
            let speed = 1.0 + layer_f * 0.3;
            let phase = layer_f * 0.5;

            let intensity = match layer {
                0 => 0,
                1 => 1,
                2 => 1,
                3 => 2,
                _ => 3,
            };
            let (color, bold) = scheme_color(state.color_scheme(), intensity, layer == NUM_LAYERS - 1);

            for x in 0..width as usize {
                let fx = x as f64;
                let wave_y = mid_y + amplitude * fast_sin(fx * frequency + time * speed + phase);
                let y = wave_y.round() as i32;

                if y >= 0 && y < height as i32 {
                    let char_idx = (fast_sin(fx * 0.3 + time * 2.0).abs() * (wave_chars.len() - 1) as f64) as usize;
                    let ch = wave_chars[char_idx.min(wave_chars.len() - 1)];
                    term.set(x as i32, y, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        time += state.speed as f64;
        term.sleep(state.speed);
    }

    Ok(())
}
