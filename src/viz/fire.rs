//! Fire effect visualization (doom-style)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

// Fire generation constants
const FIRE_GENERATION_PROBABILITY: f64 = 0.8;
const HEAT_RANGE_MIN: u8 = 200;
const HEAT_RANGE_MAX: u8 = 255;
const DECAY_RANGE_MAX: u16 = 15;
const INTENSITY_DIVISOR: u8 = 64;
const INTENSITY_MAX: u8 = 3;
const HOT_THRESHOLD: u8 = 200;

/// Run the fire effect visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step, "");

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut fire: Vec<Vec<u8>> = vec![vec![0; w]; h];
    let fire_chars = [' ', '.', ':', ';', '*', 'o', 'O', '#', '@', '%'];

    loop {
        let (new_w, new_h) = term.size();
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            fire = vec![vec![0; w]; h];
        }

        // Guard against zero-dimension terminal
        if w == 0 || h == 0 {
            term.sleep(0.1);
            continue;
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
            fire[h - 1][x] = if rng.gen_bool(FIRE_GENERATION_PROBABILITY) {
                HEAT_RANGE_MAX
            } else {
                rng.gen_range(HEAT_RANGE_MIN..HEAT_RANGE_MAX)
            };
        }

        // Propagate fire upward
        for y in 0..h - 1 {
            for x in 0..w {
                let below = fire[y + 1][x] as u16;
                let left = if x > 0 { fire[y + 1][x - 1] as u16 } else { below };
                let right = if x < w - 1 { fire[y + 1][x + 1] as u16 } else { below };

                let avg = (below + left + right) / 3;
                let decay = rng.gen_range(0..DECAY_RANGE_MAX);
                fire[y][x] = avg.saturating_sub(decay).min(HEAT_RANGE_MAX as u16) as u8;
            }
        }

        // Draw to back buffer
        for (y, row) in fire.iter().enumerate() {
            for (x, &heat) in row.iter().enumerate() {
                let char_idx = (heat as usize * (fire_chars.len() - 1)) / 255;
                let ch = fire_chars[char_idx.min(fire_chars.len() - 1)];
                let intensity = (heat / INTENSITY_DIVISOR).min(INTENSITY_MAX);
                let (color, bold) = scheme_color(state.color_scheme(), intensity, heat > HOT_THRESHOLD);
                term.set(x as i32, y as i32, ch, Some(color), bold);
            }
        }

        state.render_help(term, w as u16, h as u16);
        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
