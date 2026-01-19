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
pub mod lissajous;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::colors::ColorState;

// Re-export scheme_color from colors module for viz users
pub use crate::colors::scheme_color;

/// Default color scheme for all visualizations (7 = mono/white)
const DEFAULT_COLOR_SCHEME: u8 = 7;

/// Runtime state for interactive controls (shared by all visualizations)
pub struct VizState {
    pub speed: f32,        // Current speed (time per frame)
    pub colors: ColorState, // Color scheme state (delegated)
    pub paused: bool,
}

impl VizState {
    pub fn new(initial_speed: f32) -> Self {
        Self {
            speed: initial_speed,
            colors: ColorState::new(DEFAULT_COLOR_SCHEME),
            paused: false,
        }
    }

    /// Get current color scheme (convenience accessor)
    pub fn color_scheme(&self) -> u8 {
        self.colors.scheme
    }

    /// Handle keypress, returns true if should quit
    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> bool {
        // Try color key handling first
        if self.colors.handle_key(code) {
            return false;
        }

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
            _ => {}
        }
        false
    }
}

