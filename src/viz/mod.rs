//! Visualization modules
//!
//! Each visualization is its own module with a `run()` function.

pub mod invaders;
pub mod matrix;
pub mod life;
pub mod plasma;
pub mod fire;
pub mod rain;
pub mod waves;
pub mod cube;
pub mod hypercube;
pub mod pipes;
pub mod donut;
pub mod globe;
pub mod hex;
pub mod keyboard;
pub mod clock;
pub mod pong;
pub mod dygma;
pub mod sunlight;
pub mod audio;

use crossterm::event::{KeyCode, KeyModifiers};

// Re-export scheme_color from colors module for viz users
pub use crate::colors::scheme_color;

/// Runtime state for interactive controls (shared by all visualizations)
pub struct VizState {
    pub speed: f32,        // Current speed (time per frame)
    pub color_scheme: u8,  // Current color scheme (0-9)
    pub paused: bool,
}

impl VizState {
    pub fn new(initial_speed: f32) -> Self {
        Self {
            speed: initial_speed,
            color_scheme: 0,
            paused: false,
        }
    }

    /// Handle keypress, returns true if should quit
    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char(' ') => self.paused = !self.paused,
            // Number keys: change speed (1=fastest, 9=slowest, 0=very slow)
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as u8;
                self.speed = match n {
                    0 => 0.2,
                    1 => 0.005,
                    2 => 0.01,
                    3 => 0.02,
                    4 => 0.03,
                    5 => 0.05,
                    6 => 0.07,
                    7 => 0.1,
                    8 => 0.15,
                    9 => 0.2,
                    _ => self.speed,
                };
            }
            // Shift+number produces symbols - use these for color schemes
            KeyCode::Char('!') => self.color_scheme = 1,  // Shift+1: fire
            KeyCode::Char('@') => self.color_scheme = 2,  // Shift+2: ice
            KeyCode::Char('#') => self.color_scheme = 3,  // Shift+3: pink
            KeyCode::Char('$') => self.color_scheme = 4,  // Shift+4: gold
            KeyCode::Char('%') => self.color_scheme = 5,  // Shift+5: electric
            KeyCode::Char('^') => self.color_scheme = 6,  // Shift+6: lava
            KeyCode::Char('&') => self.color_scheme = 7,  // Shift+7: mono
            KeyCode::Char('*') => self.color_scheme = 8,  // Shift+8: rainbow
            KeyCode::Char('(') => self.color_scheme = 9,  // Shift+9: neon
            KeyCode::Char(')') => self.color_scheme = 0,  // Shift+0: green/matrix
            _ => {}
        }
        false
    }
}

