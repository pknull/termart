//! Rain effect visualization (falling raindrops with splashes)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

// Rain generation constants
const SPAWN_PROBABILITY: f64 = 0.4;
const CHAR_PROBABILITY: f64 = 0.7;
const MIN_SPEED: f32 = 0.5;
const MAX_SPEED: f32 = 2.0;

struct Raindrop {
    x: usize,
    y: f32,
    speed: f32,
    char: char,
}

struct Splash {
    x: usize,
    y: usize,
    age: u8,
}

/// Run the rain effect visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut drops: Vec<Raindrop> = Vec::new();
    let mut splashes: Vec<Splash> = Vec::new();
    let mut screen: Vec<Vec<char>> = vec![vec![' '; w]; h];

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            screen = vec![vec![' '; w]; h];
            drops.clear();
            splashes.clear();
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

        for row in &mut screen {
            row.fill(' ');
        }

        // Guard against zero-size terminal
        if w == 0 || h == 0 {
            term.sleep(0.1);
            continue;
        }

        if rng.gen_bool(SPAWN_PROBABILITY) {
            drops.push(Raindrop {
                x: rng.gen_range(0..w),
                y: 0.0,
                speed: rng.gen_range(MIN_SPEED..MAX_SPEED),
                char: if rng.gen_bool(CHAR_PROBABILITY) { '|' } else { '/' },
            });
        }

        drops.retain_mut(|drop| {
            drop.y += drop.speed;
            let y = drop.y as usize;
            if y >= h - 1 {
                splashes.push(Splash { x: drop.x, y: h - 1, age: 0 });
                false
            } else {
                if y < h && drop.x < w {
                    screen[y][drop.x] = drop.char;
                }
                true
            }
        });

        const SPLASH_CHARS: [char; 3] = ['~', '.', ' '];
        splashes.retain_mut(|splash| {
            if (splash.age as usize) < SPLASH_CHARS.len() && splash.y < h && splash.x < w {
                let ch = SPLASH_CHARS[splash.age as usize];
                if splash.x > 0 {
                    screen[splash.y][splash.x - 1] = ch;
                }
                screen[splash.y][splash.x] = ch;
                if splash.x < w - 1 {
                    screen[splash.y][splash.x + 1] = ch;
                }
                splash.age += 1;
                (splash.age as usize) < SPLASH_CHARS.len()
            } else {
                false
            }
        });

        term.clear();
        for (y, row) in screen.iter().enumerate() {
            for (x, &ch) in row.iter().enumerate() {
                if ch != ' ' {
                    let intensity = match ch {
                        '|' | '/' => 2,
                        '~' => 1,
                        _ => 0,
                    };
                    let (color, bold) = scheme_color(state.color_scheme(), intensity, ch == '|' || ch == '/');
                    term.set(x as i32, y as i32, ch, Some(color), bold);
                }
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
