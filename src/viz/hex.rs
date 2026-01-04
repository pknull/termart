//! Hexagon grid with wave/pulse animations (eDEX-UI style)

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use crossterm::style::Color;
use rand::prelude::*;
use std::io;

struct Pulse {
    x: f32,
    y: f32,
    birth_time: f32,
    speed: f32,
    max_radius: f32,
}

/// Run the hex grid visualization
pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);
    state.color_scheme = 5; // Default to electric (cyan/white)

    let mut time: f32 = 0.0;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Hex cell dimensions (in characters)
    // Using flat-top hexagons:  / \
    //                          |   |
    //                           \ /
    let hex_width: usize = 6;  // Width of one hex cell
    let hex_height: usize = 3; // Height of one hex cell

    let mut pulses: Vec<Pulse> = Vec::new();

    // Precompute sin table
    const SIN_TABLE_SIZE: usize = 1024;
    let sin_table: Vec<f32> = (0..SIN_TABLE_SIZE)
        .map(|i| ((i as f32 / SIN_TABLE_SIZE as f32) * std::f32::consts::TAU).sin())
        .collect();

    let fast_sin = |x: f32| -> f32 {
        let normalized = x.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let idx = (normalized * SIN_TABLE_SIZE as f32) as usize;
        sin_table[idx.min(SIN_TABLE_SIZE - 1)]
    };

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;
            pulses.clear();
        }

        let w = width as usize;
        let h = height as usize;

        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                break;
            }
        }

        if state.paused {
            term.sleep(0.1);
            continue;
        }

        // Spawn new pulses randomly
        if rng.gen_bool(0.08) {
            pulses.push(Pulse {
                x: rng.gen_range(0.0..w as f32),
                y: rng.gen_range(0.0..h as f32),
                birth_time: time,
                speed: rng.gen_range(15.0..35.0),
                max_radius: rng.gen_range(30.0..80.0),
            });
        }

        // Remove old pulses
        pulses.retain(|p| {
            let age = time - p.birth_time;
            age * p.speed < p.max_radius + 10.0
        });

        term.clear();

        // Calculate hex grid dimensions - extend beyond screen for edge hexes
        let cols = (w / hex_width) + 3;
        let rows = (h / hex_height) + 3;

        // Helper to safely set a character (clips to screen bounds)
        let set_if_visible = |term: &mut Terminal, x: i32, y: i32, ch: char, color: Color, bold: bool| {
            if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                term.set(x, y, ch, Some(color), bold);
            }
        };

        // Draw hexagon grid - start before screen to catch partial hexes
        for row in 0..rows {
            for col in 0..cols {
                // Offset every other row for honeycomb pattern
                let x_offset: i32 = if row % 2 == 1 { (hex_width / 2) as i32 } else { 0 };
                let cx = (col as i32 * hex_width as i32) + x_offset - (hex_width as i32);
                let cy = (row as i32 * hex_height as i32) - (hex_height as i32);

                // Calculate intensity from pulses and ambient waves
                let mut intensity: f32 = 0.0;

                // Use center for wave calculations (clamp to valid range for visual continuity)
                let wave_x = cx.max(0) as f32;
                let wave_y = cy.max(0) as f32;

                // Ambient wave pattern
                let wave1 = fast_sin((wave_x * 0.1) + time * 2.0) * 0.3;
                let wave2 = fast_sin((wave_y * 0.15) + time * 1.5) * 0.2;
                let wave3 = fast_sin(((wave_x + wave_y) * 0.08) - time * 1.2) * 0.2;
                intensity += (wave1 + wave2 + wave3 + 0.7).max(0.0);

                // Pulse contributions
                for pulse in &pulses {
                    let dx = cx as f32 - pulse.x;
                    let dy = (cy as f32 - pulse.y) * 2.0; // Adjust for terminal aspect
                    let dist = (dx * dx + dy * dy).sqrt();
                    let age = time - pulse.birth_time;
                    let ring_pos = age * pulse.speed;

                    // Ring wave effect
                    let ring_dist = (dist - ring_pos).abs();
                    if ring_dist < 5.0 && dist < pulse.max_radius {
                        let ring_intensity = (1.0 - ring_dist / 5.0) * (1.0 - dist / pulse.max_radius);
                        intensity += ring_intensity * 1.5;
                    }
                }

                intensity = intensity.clamp(0.0, 1.5);

                // Draw hex based on intensity
                if intensity > 0.1 {
                    let level = ((intensity * 4.0) as u8).min(3);
                    let (color, bold) = scheme_color(state.color_scheme, level, intensity > 0.8);

                    // Draw hexagon shape - let set_if_visible handle clipping
                    // Top:    /_\
                    // Mid:   |   |
                    // Bot:    \_/
                    let x = cx - 2;
                    let y = cy - 1;

                    if intensity > 0.3 {
                        // Full hex - no top underscore to avoid doubling
                        //  / \
                        // |   |
                        //  \_/
                        set_if_visible(term, x + 1, y, '/', color, bold);
                        set_if_visible(term, x + 3, y, '\\', color, bold);
                        set_if_visible(term, x, y + 1, '|', color, bold);
                        set_if_visible(term, x + 4, y + 1, '|', color, bold);
                        set_if_visible(term, x + 1, y + 2, '\\', color, bold);
                        set_if_visible(term, x + 2, y + 2, '_', color, bold);
                        set_if_visible(term, x + 3, y + 2, '/', color, bold);
                    } else {
                        // Dim hex - just corners
                        set_if_visible(term, x + 1, y, '.', color, bold);
                        set_if_visible(term, x + 3, y, '.', color, bold);
                        set_if_visible(term, x + 1, y + 2, '.', color, bold);
                        set_if_visible(term, x + 3, y + 2, '.', color, bold);
                    }
                }
            }
        }

        term.present()?;
        time += state.speed * 2.0;
        term.sleep(state.speed);
    }

    Ok(())
}
