//! Matrix rain effect (cmatrix-like)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use crate::viz::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

// Character set for matrix rain - half-width katakana + latin + digits + symbols
// Half-width katakana (U+FF66-FF9D) is what the original Matrix used
const CHARS: &[char] = &[
    // Half-width katakana
    'ｦ', 'ｧ', 'ｨ', 'ｩ', 'ｪ', 'ｫ', 'ｬ', 'ｭ', 'ｮ', 'ｯ', 'ｰ', 'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ',
    'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ',
    'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ',
    'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ',
    // Digits
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    // Latin
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    // Symbols
    '@', '#', '$', '%', '&', '*', '+', '-', '=', '<', '>', '?',
];
const CHARS_LEN: usize = CHARS.len();

// Drop configuration constants
const DROP_LENGTH_MIN: usize = 5;
const DROP_LENGTH_MAX: usize = 20;
const CHAR_REFRESH_PROBABILITY: f64 = 0.3;

// Frame-skip speed tiers (cmatrix-style): column advances when frame % update_rate == 0
// Lower = faster. Range 2-5 gives visible speed variation without any column moving every frame.
const UPDATE_RATE_MIN: u8 = 2;
const UPDATE_RATE_MAX: u8 = 5;

// Glitch effect: random character corruption
const GLITCH_PROBABILITY: f64 = 0.02;  // Per-character per-frame chance to glitch

struct Drop {
    y: i32,
    update_rate: u8,  // Frame-skip threshold (1=fastest, 4=slowest)
    length: usize,
    chars: [char; 25],
    glitch_char: Option<(usize, char)>,  // (position in trail, replacement char)
}

impl Drop {
    #[inline]
    fn new(rng: &mut StdRng, h: usize) -> Self {
        let mut chars = [' '; 25];
        for c in &mut chars {
            *c = CHARS[rng.gen_range(0..CHARS_LEN)];
        }
        // Scatter initial positions across full screen for staggered start
        let safe_h = h.max(1) as i32;
        Self {
            y: rng.gen_range(-safe_h..safe_h),
            update_rate: rng.gen_range(UPDATE_RATE_MIN..=UPDATE_RATE_MAX),
            length: rng.gen_range(DROP_LENGTH_MIN..DROP_LENGTH_MAX),
            chars,
            glitch_char: None,
        }
    }

    #[inline]
    fn reset(&mut self, rng: &mut StdRng) {
        self.y = rng.gen_range(-(self.length as i32)..0);
        self.update_rate = rng.gen_range(UPDATE_RATE_MIN..=UPDATE_RATE_MAX);
        self.length = rng.gen_range(DROP_LENGTH_MIN..DROP_LENGTH_MAX);
        self.glitch_char = None;
        for c in &mut self.chars {
            if rng.gen_bool(CHAR_REFRESH_PROBABILITY) {
                *c = CHARS[rng.gen_range(0..CHARS_LEN)];
            }
        }
    }

    /// Maybe trigger a glitch — temporarily corrupt a random character in the trail
    #[inline]
    fn maybe_glitch(&mut self, rng: &mut StdRng) {
        if rng.gen_bool(GLITCH_PROBABILITY) {
            let pos = rng.gen_range(0..self.length);
            let ch = CHARS[rng.gen_range(0..CHARS_LEN)];
            self.glitch_char = Some((pos, ch));
        } else if self.glitch_char.is_some() && rng.gen_bool(0.08) {
            // 8% chance to clear glitch each frame — glitches last ~12 frames on average
            self.glitch_char = None;
        }
    }
}

pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step, "");

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut drops: Vec<Drop> = (0..w)
        .map(|_| Drop::new(rng, h))
        .collect();

    let mut frame: u8 = 0;

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

        // Render all drops
        for (x, drop) in drops.iter().enumerate() {
            let head_y = drop.y;
            let len = drop.length;
            let half_len = len / 2;

            for i in 0..len {
                let y = head_y - i as i32;
                if y >= 0 && y < h as i32 {
                    let char_idx = (y as usize + x) % drop.chars.len();
                    // Check for glitch override at this position
                    let is_glitched = drop.glitch_char.map_or(false, |(pos, _)| i == pos);
                    let ch = if let Some((glitch_pos, glitch_ch)) = drop.glitch_char {
                        if i == glitch_pos { glitch_ch } else { drop.chars[char_idx] }
                    } else {
                        drop.chars[char_idx]
                    };
                    // Glitched chars get lead color (intensity 3) to make them pop
                    let intensity = if is_glitched || i == 0 { 3 } else if i < 3 { 2 } else if i < half_len { 1 } else { 0 };
                    let (color, bold) = scheme_color(state.color_scheme(), intensity, is_glitched || i < 3);
                    term.set(x as i32, y, ch, Some(color), bold);
                }
            }
        }

        state.render_help(term, w as u16, h as u16);
        term.present()?;

        // Advance drops based on frame-skip (cmatrix-style async)
        frame = frame.wrapping_add(1);
        for drop in &mut drops {
            // Glitch effect runs every frame regardless of movement
            drop.maybe_glitch(rng);

            // Column advances only when frame counter is divisible by its update_rate
            if frame % drop.update_rate == 0 {
                drop.y += 1;
                if drop.y - drop.length as i32 >= h as i32 {
                    drop.reset(rng);
                }
            }
        }

        term.sleep(state.speed);
    }

    Ok(())
}
