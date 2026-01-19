//! Conway's Game of Life visualization

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use rand::prelude::*;
use std::io;

/// Initial probability of a cell being alive when grid is created
const INITIAL_DENSITY: f64 = 0.3;
/// Number of generations between periodic life injections
const INJECTION_INTERVAL: u64 = 100;
/// Divisor for calculating injection count (w * h / INJECTION_DIVISOR)
const INJECTION_DIVISOR: usize = 50;

/// Run the Game of Life visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng, draw_char: char) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut grid: Vec<Vec<bool>> = (0..h)
        .map(|_| (0..w).map(|_| rng.gen_bool(INITIAL_DENSITY)).collect())
        .collect();

    let mut next_grid = grid.clone();
    let mut neighbor_counts: Vec<Vec<u8>> = vec![vec![0; w]; h];
    let mut generation = 0u64;

    loop {
        // Check for terminal resize - uses crossterm directly to get fresh size
        // rather than term.size() which returns the cached internal dimensions
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            grid = (0..h)
                .map(|_| (0..w).map(|_| rng.gen_bool(INITIAL_DENSITY)).collect())
                .collect();
            next_grid = grid.clone();
            neighbor_counts = vec![vec![0; w]; h];
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

        // Compute neighbor counts
        for (y, row) in neighbor_counts.iter_mut().enumerate() {
            for (x, count) in row.iter_mut().enumerate() {
                *count = count_neighbors(&grid, x, y, w, h);
            }
        }

        // Clear and populate back buffer
        term.clear();
        for (y, (grid_row, count_row)) in grid.iter().zip(neighbor_counts.iter()).enumerate() {
            for (x, (&alive, &neighbors)) in grid_row.iter().zip(count_row.iter()).enumerate() {

                if alive {
                    let intensity = match neighbors { 2 => 1, 3 => 2, _ => 0 };
                    let (color, bold) = scheme_color(state.color_scheme(), intensity, false);
                    term.set(x as i32, y as i32, draw_char, Some(color), bold);
                }

                next_grid[y][x] = matches!((alive, neighbors), (true, 2) | (true, 3) | (false, 3));
            }
        }

        term.present()?;
        term.sleep(state.speed);

        std::mem::swap(&mut grid, &mut next_grid);
        generation += 1;

        // Periodically inject new life to prevent stagnation
        if generation % INJECTION_INTERVAL == 0 && w > 0 && h > 0 {
            for _ in 0..((w * h) / INJECTION_DIVISOR) {
                let x = rng.gen_range(0..w);
                let y = rng.gen_range(0..h);
                grid[y][x] = true;
            }
        }
    }

    Ok(())
}

#[inline]
fn count_neighbors(grid: &[Vec<bool>], x: usize, y: usize, w: usize, h: usize) -> u8 {
    let mut count = 0u8;
    let xi = x as i32;
    let yi = y as i32;
    let wi = w as i32;
    let hi = h as i32;
    // Unrolled loop for all 8 neighbors
    let neighbors = [
        ((xi - 1).rem_euclid(wi) as usize, (yi - 1).rem_euclid(hi) as usize),
        (x, (yi - 1).rem_euclid(hi) as usize),
        ((xi + 1).rem_euclid(wi) as usize, (yi - 1).rem_euclid(hi) as usize),
        ((xi - 1).rem_euclid(wi) as usize, y),
        ((xi + 1).rem_euclid(wi) as usize, y),
        ((xi - 1).rem_euclid(wi) as usize, (yi + 1).rem_euclid(hi) as usize),
        (x, (yi + 1).rem_euclid(hi) as usize),
        ((xi + 1).rem_euclid(wi) as usize, (yi + 1).rem_euclid(hi) as usize),
    ];
    for (nx, ny) in neighbors {
        if grid[ny][nx] { count += 1; }
    }
    count
}
