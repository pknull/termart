use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Color;

/// Shared color scheme state
#[derive(Clone, Copy)]
pub struct ColorState {
    pub scheme: u8,
}

impl ColorState {
    pub fn new(default_scheme: u8) -> Self {
        Self {
            scheme: default_scheme,
        }
    }

    /// Handle color scheme key input. Returns true if key was handled.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let scheme = match code {
            KeyCode::Char('!') => 1, // Shift+1: fire
            KeyCode::Char('@') => 2, // Shift+2: ice
            KeyCode::Char('#') => 3, // Shift+3: pink
            KeyCode::Char('$') => 4, // Shift+4: gold
            KeyCode::Char('%') => 5, // Shift+5: electric
            KeyCode::Char('^') => 6, // Shift+6: lava
            KeyCode::Char('&') => 7, // Shift+7: mono
            KeyCode::Char('*') => 8, // Shift+8: rainbow
            KeyCode::Char('(') => 9, // Shift+9: neon
            KeyCode::Char(')') => 0, // Shift+0: green/matrix
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_digit() => {
                c as u8 - b'0'
            }
            _ => return false,
        };
        self.scheme = scheme;
        true
    }

    /// Check if using mono/semantic color mode
    pub fn is_mono(&self) -> bool {
        self.scheme == 7
    }

    pub fn name(&self) -> &'static str {
        match self.scheme {
            0 => "Matrix",
            1 => "Fire",
            2 => "Ice",
            3 => "Pink",
            4 => "Gold",
            5 => "Electric",
            6 => "Lava",
            7 => "Mono",
            8 => "Rainbow",
            9 => "Neon",
            _ => "Unknown",
        }
    }
}

/// Get color from scheme based on intensity (0-3)
pub fn scheme_color(scheme: u8, intensity: u8, bold: bool) -> (Color, bool) {
    match scheme {
        1 => match intensity {
            // Red/Yellow (fire)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::DarkYellow, bold),
            _ => (Color::Yellow, true),
        },
        2 => match intensity {
            // Blue/Cyan (ice)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::Cyan, bold),
            _ => (Color::Cyan, true),
        },
        3 => match intensity {
            // Magenta/Pink (pink)
            0 => (Color::DarkMagenta, false),
            1 => (Color::Magenta, false),
            2 => (Color::Magenta, bold),
            _ => (Color::AnsiValue(13), true), // Bright magenta
        },
        4 => match intensity {
            // Yellow/Gold (gold)
            0 => (Color::DarkYellow, false),
            1 => (Color::Yellow, false),
            2 => (Color::Yellow, bold),
            _ => (Color::AnsiValue(11), true), // Bright yellow
        },
        5 => match intensity {
            // Cyan/Electric (electric)
            0 => (Color::DarkCyan, false),
            1 => (Color::Cyan, false),
            2 => (Color::Cyan, bold),
            _ => (Color::AnsiValue(14), true), // Bright cyan
        },
        6 => match intensity {
            // Red/Magenta (lava)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::Magenta, bold),
            _ => (Color::AnsiValue(9), true), // Bright red
        },
        7 => match intensity {
            // White/Grey (mono)
            0 => (Color::DarkGrey, false),
            1 => (Color::Grey, false),
            2 => (Color::White, bold),
            _ => (Color::White, true),
        },
        8 => match intensity {
            // Rainbow cycling
            0 => (Color::Red, false),
            1 => (Color::Yellow, false),
            2 => (Color::Green, bold),
            _ => (Color::Cyan, true),
        },
        9 => match intensity {
            // Blue/Magenta (neon)
            0 => (Color::DarkBlue, false),
            1 => (Color::Blue, false),
            2 => (Color::Magenta, bold),
            _ => (Color::AnsiValue(13), true), // Bright magenta
        },
        _ => match intensity {
            // Default: Green (matrix)
            0 => (Color::DarkGreen, false),
            1 => (Color::Green, false),
            2 => (Color::Green, true),
            _ => (Color::AnsiValue(10), true), // Bright green
        },
    }
}

#[cfg(test)]
mod tests {
    use super::ColorState;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn color_shortcuts_accept_both_shifted_key_representations() {
        let symbols = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];
        let mut colors = ColorState::new(7);

        for (scheme, symbol) in symbols.into_iter().enumerate() {
            assert!(colors.handle_key(KeyCode::Char(symbol), KeyModifiers::NONE));
            assert_eq!(colors.scheme, scheme as u8);

            let digit = char::from_digit(scheme as u32, 10).expect("valid test digit");
            colors.scheme = 7;
            assert!(colors.handle_key(KeyCode::Char(digit), KeyModifiers::SHIFT));
            assert_eq!(colors.scheme, scheme as u8);
        }
    }

    #[test]
    fn unmodified_digits_remain_available_for_speed_controls() {
        let mut colors = ColorState::new(7);
        assert!(!colors.handle_key(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(colors.scheme, 7);
    }

    #[test]
    fn schemes_have_stable_display_names() {
        let expected = [
            "Matrix", "Fire", "Ice", "Pink", "Gold", "Electric", "Lava", "Mono", "Rainbow", "Neon",
        ];

        for (scheme, name) in expected.into_iter().enumerate() {
            assert_eq!(ColorState::new(scheme as u8).name(), name);
        }
    }
}
