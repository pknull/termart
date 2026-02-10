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
pub mod tui_cover;
pub mod tui_control;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::colors::ColorState;
use crate::help::render_help_overlay;
use crate::terminal::Terminal;

// Re-export scheme_color from colors module for viz users
pub use crate::colors::scheme_color;

/// Default color scheme for all visualizations (7 = mono/white)
const DEFAULT_COLOR_SCHEME: u8 = 7;

/// Global help section appended to all visualizer help text
const GLOBAL_HELP: &str = "\
───────────────────────
 GLOBAL CONTROLS
 Space   Pause/resume
 1-9     Speed (1=fast)
 !-()    Color scheme
 q/Esc   Quit
 ?       Close help
───────────────────────";

/// Runtime state for interactive controls (shared by all visualizations)
pub struct VizState {
    pub speed: f32,        // Current speed (time per frame)
    pub colors: ColorState, // Color scheme state (delegated)
    pub paused: bool,
    pub show_help: bool,   // Whether help overlay is visible
    help_text: &'static str, // Visualizer-specific help text
}

impl VizState {
    pub fn new(initial_speed: f32, help_text: &'static str) -> Self {
        Self {
            speed: initial_speed,
            colors: ColorState::new(DEFAULT_COLOR_SCHEME),
            paused: false,
            show_help: false,
            help_text,
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
            KeyCode::Char('?') => self.show_help = !self.show_help,
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

    /// Render help overlay centered on screen
    pub fn render_help(&self, term: &mut Terminal, width: u16, height: u16) {
        if !self.show_help {
            return;
        }

        // Build combined help text
        let combined = if self.help_text.is_empty() {
            GLOBAL_HELP.to_string()
        } else {
            format!("{}\n{}", self.help_text, GLOBAL_HELP)
        };

        render_help_overlay(term, width, height, &combined);
    }
}

#[cfg(test)]
mod tests {
    use super::VizState;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn handle_key_toggles_help() {
        let mut state = VizState::new(0.03, "");
        assert!(!state.show_help);
        let quit = state.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(!quit);
        assert!(state.show_help);
    }

    #[test]
    fn handle_key_quit() {
        let mut state = VizState::new(0.03, "");
        let quit = state.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(quit);
    }

    #[test]
    fn handle_key_speed_presets() {
        let mut state = VizState::new(0.03, "");
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!((state.speed - 0.005).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('0'), KeyModifiers::NONE);
        assert!((state.speed - 0.2).abs() < f32::EPSILON);
    }
}
