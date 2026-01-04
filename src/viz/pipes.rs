//! Classic pipes screensaver visualization

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

struct Pipe {
    x: i32,
    y: i32,
    dir: u8,
    steps: u32,
}

/// Run the pipes visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let pipe_chars: [[char; 4]; 4] = [
        ['│', '└', '│', '┘'],
        ['┘', '─', '┐', '─'],
        ['│', '┌', '│', '┐'],
        ['└', '─', '┌', '─'],
    ];

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut pipes: Vec<Pipe> = Vec::new();
    let mut fill_count: usize = 0;

    let spawn_pipe = |rng: &mut StdRng, w: usize, h: usize| -> Pipe {
        let dir = rng.gen_range(0..4);
        let (x, y) = match dir {
            0 => (rng.gen_range(0..w as i32), h as i32 - 1),
            1 => (0, rng.gen_range(0..h as i32)),
            2 => (rng.gen_range(0..w as i32), 0),
            _ => (w as i32 - 1, rng.gen_range(0..h as i32)),
        };
        Pipe { x, y, dir, steps: 0 }
    };

    for _ in 0..5 {
        pipes.push(spawn_pipe(rng, w, h));
    }

    loop {
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            fill_count = 0;
            pipes.clear();
            for _ in 0..5 {
                pipes.push(spawn_pipe(rng, w, h));
            }
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

        if fill_count > (w * h * 7) / 10 {
            term.clear_screen()?;
            fill_count = 0;
            pipes.clear();
            for _ in 0..5 {
                pipes.push(spawn_pipe(rng, w, h));
            }
        }

        for pipe in &mut pipes {
            let old_dir = pipe.dir;
            if pipe.steps > 3 && rng.gen_bool(0.25) {
                pipe.dir = if rng.gen_bool(0.5) {
                    (pipe.dir + 1) % 4
                } else {
                    (pipe.dir + 3) % 4
                };
                pipe.steps = 0;
            }

            let ch = pipe_chars[old_dir as usize][pipe.dir as usize];

            if pipe.x >= 0 && pipe.x < w as i32 && pipe.y >= 0 && pipe.y < h as i32 {
                fill_count += 1;
                let (color, bold) = scheme_color(state.color_scheme, 2, true);
                term.set(pipe.x, pipe.y, ch, Some(color), bold);
            }

            match pipe.dir {
                0 => pipe.y -= 1,
                1 => pipe.x += 1,
                2 => pipe.y += 1,
                _ => pipe.x -= 1,
            }
            pipe.steps += 1;

            if pipe.x < 0 || pipe.x >= w as i32 || pipe.y < 0 || pipe.y >= h as i32 {
                *pipe = spawn_pipe(rng, w, h);
            }
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
