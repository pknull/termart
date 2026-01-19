//! Matrix rain effect (cmatrix-like)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use crate::viz::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

// Character set for matrix rain
const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789@#$%^&*(){}[]|;:,.<>?~`";
const CHARS_LEN: usize = CHARS.len();

// Drop configuration constants
const DROP_SPEED_MIN: f32 = 0.5;
const DROP_SPEED_MAX: f32 = 1.2;
const DROP_LENGTH_MIN: usize = 5;
const DROP_LENGTH_MAX: usize = 20;
const CHAR_REFRESH_PROBABILITY: f64 = 0.3;

struct Drop {
    y: f32,
    speed: f32,
    length: usize,
    chars: [char; 25],  // Fixed-size array instead of Vec
}

impl Drop {
    #[inline]
    fn new(rng: &mut StdRng, h: usize) -> Self {
        let mut chars = [' '; 25];
        for c in &mut chars {
            *c = CHARS[rng.gen_range(0..CHARS_LEN)] as char;
        }
        // Use h.max(1) to prevent empty range panic when h=0
        let safe_h = h.max(1) as f32;
        Self {
            y: rng.gen_range(-safe_h..0.0),
            speed: rng.gen_range(DROP_SPEED_MIN..DROP_SPEED_MAX),
            length: rng.gen_range(DROP_LENGTH_MIN..DROP_LENGTH_MAX),
            chars,
        }
    }

    #[inline]
    fn reset(&mut self, rng: &mut StdRng) {
        // Use length-based reset range for consistency with new()
        let reset_range = -(self.length as f32 * 1.5);
        self.y = rng.gen_range(reset_range..0.0);
        self.speed = rng.gen_range(DROP_SPEED_MIN..DROP_SPEED_MAX);
        self.length = rng.gen_range(DROP_LENGTH_MIN..DROP_LENGTH_MAX);
        // Only randomize some chars
        for c in &mut self.chars {
            if rng.gen_bool(CHAR_REFRESH_PROBABILITY) {
                *c = CHARS[rng.gen_range(0..CHARS_LEN)] as char;
            }
        }
    }
}

pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut drops: Vec<Drop> = (0..w)
        .map(|_| Drop::new(rng, h))
        .collect();

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            drops.resize_with(w, || Drop::new(rng, h));
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

        term.clear();

        for (x, drop) in drops.iter().enumerate() {
            let head_y = drop.y as i32;
            let len = drop.length;
            let half_len = len / 2;

            for i in 0..len {
                let y = head_y - i as i32;
                if y >= 0 && y < h as i32 {
                    let char_idx = (y as usize + x) % drop.chars.len();
                    let ch = drop.chars[char_idx];
                    let intensity = if i == 0 { 3 } else if i < 3 { 2 } else if i < half_len { 1 } else { 0 };
                    let (color, bold) = scheme_color(state.color_scheme(), intensity, i < 3);
                    term.set(x as i32, y, ch, Some(color), bold);
                }
            }
        }

        term.present()?;

        for drop in &mut drops {
            drop.y += drop.speed;
            if drop.y as i32 - drop.length as i32 >= h as i32 {
                drop.reset(rng);
            }
        }

        term.sleep(state.speed);
    }

    Ok(())
}
