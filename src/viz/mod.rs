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
use crossterm::style::Color;
use crate::colors::ColorState;
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

        let lines: Vec<&str> = combined.lines().collect();
        let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let box_width = max_width + 4; // 2 chars padding each side
        let box_height = lines.len() + 2; // 1 row padding top/bottom

        // Center the box
        let start_x = (width as usize).saturating_sub(box_width) / 2;
        let start_y = (height as usize).saturating_sub(box_height) / 2;

        let border_color = Color::White;
        let text_color = Color::Grey;

        // Draw top border: ┌─────┐
        term.set(start_x as i32, start_y as i32, '┌', Some(border_color), false);
        for x in 1..box_width - 1 {
            term.set((start_x + x) as i32, start_y as i32, '─', Some(border_color), false);
        }
        term.set((start_x + box_width - 1) as i32, start_y as i32, '┐', Some(border_color), false);

        // Draw content rows with side borders
        for (i, line) in lines.iter().enumerate() {
            let y = start_y + 1 + i;
            term.set(start_x as i32, y as i32, '│', Some(border_color), false);

            // Pad line to fill box
            let padding = max_width.saturating_sub(line.chars().count());
            let padded = format!(" {}{} ", line, " ".repeat(padding));
            for (j, ch) in padded.chars().enumerate() {
                term.set((start_x + 1 + j) as i32, y as i32, ch, Some(text_color), false);
            }

            term.set((start_x + box_width - 1) as i32, y as i32, '│', Some(border_color), false);
        }

        // Draw bottom border: └─────┘
        let bottom_y = start_y + box_height - 1;
        term.set(start_x as i32, bottom_y as i32, '└', Some(border_color), false);
        for x in 1..box_width - 1 {
            term.set((start_x + x) as i32, bottom_y as i32, '─', Some(border_color), false);
        }
        term.set((start_x + box_width - 1) as i32, bottom_y as i32, '┘', Some(border_color), false);
    }
}

