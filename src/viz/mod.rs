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
pub mod pipes;
pub mod donut;
pub mod globe;
pub mod hex;
pub mod keyboard;
pub mod clock;
pub mod pong;
pub mod dygma;

use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Color;

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

/// Get color based on scheme and intensity
pub fn scheme_color(scheme: u8, intensity: u8, bold: bool) -> (Color, bool) {
    match scheme {
        1 => match intensity {  // Red/Yellow (fire)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::DarkYellow, bold),
            _ => (Color::Yellow, true),
        },
        2 => match intensity {  // Blue/Cyan (ice)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::DarkCyan, bold),
            _ => (Color::Cyan, true),
        },
        3 => match intensity {  // Magenta/Red (pink)
            0 => (Color::DarkMagenta, false),
            1 => (Color::Magenta, false),
            2 => (Color::Red, bold),
            _ => (Color::White, true),
        },
        4 => match intensity {  // Yellow/White (gold)
            0 => (Color::DarkYellow, false),
            1 => (Color::Yellow, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        5 => match intensity {  // Cyan/White (electric)
            0 => (Color::DarkCyan, false),
            1 => (Color::Cyan, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        6 => match intensity {  // Red/Magenta (lava)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::Magenta, bold),
            _ => (Color::White, true),
        },
        7 => match intensity {  // White/Grey (mono)
            0 => (Color::DarkGrey, false),
            1 => (Color::Grey, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        8 => match intensity {  // Rainbow cycling
            0 => (Color::Red, false),
            1 => (Color::Yellow, false),
            2 => (Color::Green, bold),
            _ => (Color::Cyan, true),
        },
        9 => match intensity {  // Blue/Magenta (neon)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::Magenta, bold),
            _ => (Color::White, true),
        },
        _ => match intensity {  // Default: Green (matrix) - classic cmatrix look
            0 => (Color::DarkGreen, false),
            1 => (Color::Green, false),
            2 => (Color::Green, true),
            _ => (Color::White, true),  // Bright white for head
        },
    }
}
