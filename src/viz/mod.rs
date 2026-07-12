//! Visualization modules
//!
//! Each visualization is its own module with a `run()` function.

pub mod audio;
pub mod clock;
pub mod codex_tokens;
pub mod cube;
pub mod donut;
pub mod dygma;
pub mod fire;
pub mod fractal;
pub mod globe;
pub mod hex;
pub mod hypercube;
pub mod invaders;
pub mod keyboard;
pub mod life;
pub mod lissajous;
pub mod matrix;
pub mod pipes;
pub mod plasma;
pub mod pong;
pub mod rain;
pub mod sunlight;
pub mod tokeneater;
pub mod tui_control;
pub mod tui_cover;
mod usage;
pub mod waves;

use crate::colors::ColorState;
use crate::help::{render_help_spec, HelpSpec};
use crate::terminal::Terminal;
use crossterm::event::{KeyCode, KeyModifiers};

// Re-export scheme_color from colors module for viz users
pub use crate::colors::scheme_color;

/// Default color scheme for all visualizations (7 = mono/white)
const DEFAULT_COLOR_SCHEME: u8 = 7;

/// Runtime state for interactive controls (shared by all visualizations)
pub struct VizState {
    pub speed: f32,         // Current speed (time per frame)
    pub colors: ColorState, // Color scheme state (delegated)
    pub paused: bool,
    pub show_help: bool, // Whether help overlay is visible
    help: HelpSpec,
    animation_controls: bool,
}

impl VizState {
    pub fn new(initial_speed: f32, help: HelpSpec) -> Self {
        Self {
            speed: initial_speed,
            colors: ColorState::new(DEFAULT_COLOR_SCHEME),
            paused: false,
            show_help: false,
            help,
            animation_controls: true,
        }
    }

    /// Runtime state for static widgets that need colors/help/quit but have no
    /// meaningful pause or animation-speed controls.
    pub fn new_static(poll_interval: f32, help: HelpSpec) -> Self {
        Self {
            speed: poll_interval.max(0.05),
            colors: ColorState::new(DEFAULT_COLOR_SCHEME),
            paused: false,
            show_help: false,
            help,
            animation_controls: false,
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
            KeyCode::Char(' ') if self.animation_controls => self.paused = !self.paused,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            // Number keys: change speed (1=fastest, 9=slowest, 0=very slow)
            KeyCode::Char(c) if self.animation_controls && c.is_ascii_digit() => {
                let Some(n) = c.to_digit(10) else {
                    return false;
                };
                let n = n as u8;
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

        render_help_spec(term, width, height, &self.help);
    }
}

#[cfg(test)]
mod tests {
    use super::VizState;
    use crate::help::HelpSpec;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn handle_key_toggles_help() {
        let mut state = VizState::new(0.03, HelpSpec::animated("TEST", &[]));
        assert!(!state.show_help);
        let quit = state.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(!quit);
        assert!(state.show_help);
    }

    #[test]
    fn handle_key_quit() {
        let mut state = VizState::new(0.03, HelpSpec::animated("TEST", &[]));
        let quit = state.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(quit);
    }

    #[test]
    fn handle_key_speed_presets() {
        let mut state = VizState::new(0.03, HelpSpec::animated("TEST", &[]));
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!((state.speed - 0.005).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('0'), KeyModifiers::NONE);
        assert!((state.speed - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn static_state_ignores_animation_controls() {
        let mut state = VizState::new_static(0.01, HelpSpec::colored("TEST", &[]));
        assert!((state.speed - 0.05).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        state.handle_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert!((state.speed - 0.05).abs() < f32::EPSILON);
        assert!(!state.paused);
    }
}
