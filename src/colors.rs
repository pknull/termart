use crossterm::event::KeyCode;
use crossterm::style::Color;

/// Shared color scheme state
#[derive(Clone, Copy)]
pub struct ColorState {
    pub scheme: u8,
}

impl ColorState {
    pub fn new(default_scheme: u8) -> Self {
        Self { scheme: default_scheme }
    }

    /// Handle color scheme key input. Returns true if key was handled.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('!') => self.scheme = 1,  // Shift+1: fire
            KeyCode::Char('@') => self.scheme = 2,  // Shift+2: ice
            KeyCode::Char('#') => self.scheme = 3,  // Shift+3: pink
            KeyCode::Char('$') => self.scheme = 4,  // Shift+4: gold
            KeyCode::Char('%') => self.scheme = 5,  // Shift+5: electric
            KeyCode::Char('^') => self.scheme = 6,  // Shift+6: lava
            KeyCode::Char('&') => self.scheme = 7,  // Shift+7: mono
            KeyCode::Char('*') => self.scheme = 8,  // Shift+8: rainbow
            KeyCode::Char('(') => self.scheme = 9,  // Shift+9: neon
            KeyCode::Char(')') => self.scheme = 0,  // Shift+0: green/matrix
            _ => return false,
        }
        true
    }

    /// Check if using mono/semantic color mode
    pub fn is_mono(&self) -> bool {
        self.scheme == 7
    }
}

/// Get color from scheme based on intensity (0-3)
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
            2 => (Color::Cyan, bold),
            _ => (Color::Cyan, true),
        },
        3 => match intensity {  // Magenta/Pink (pink)
            0 => (Color::DarkMagenta, false),
            1 => (Color::Magenta, false),
            2 => (Color::Magenta, bold),
            _ => (Color::AnsiValue(13), true),  // Bright magenta
        },
        4 => match intensity {  // Yellow/Gold (gold)
            0 => (Color::DarkYellow, false),
            1 => (Color::Yellow, false),
            2 => (Color::Yellow, bold),
            _ => (Color::AnsiValue(11), true),  // Bright yellow
        },
        5 => match intensity {  // Cyan/Electric (electric)
            0 => (Color::DarkCyan, false),
            1 => (Color::Cyan, false),
            2 => (Color::Cyan, bold),
            _ => (Color::AnsiValue(14), true),  // Bright cyan
        },
        6 => match intensity {  // Red/Magenta (lava)
            0 => (Color::DarkRed, false),
            1 => (Color::Red, false),
            2 => (Color::Magenta, bold),
            _ => (Color::AnsiValue(9), true),  // Bright red
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
            _ => (Color::AnsiValue(13), true),  // Bright magenta
        },
        _ => match intensity {  // Default: Green (matrix)
            0 => (Color::DarkGreen, false),
            1 => (Color::Green, false),
            2 => (Color::Green, true),
            _ => (Color::AnsiValue(10), true),  // Bright green
        },
    }
}

/// Map semantic status color to scheme color
/// Used when not in mono mode to theme status indicators
pub fn status_to_scheme(scheme: u8, status: StatusColor) -> Color {
    if scheme == 7 {
        // Mono mode - use semantic colors
        match status {
            StatusColor::Good => Color::Green,
            StatusColor::Warning => Color::Yellow,
            StatusColor::Critical => Color::Red,
            StatusColor::Info => Color::Cyan,
            StatusColor::Muted => Color::DarkGrey,
        }
    } else {
        // Themed mode - map to scheme intensity
        let intensity = match status {
            StatusColor::Muted => 0,
            StatusColor::Info => 1,
            StatusColor::Good => 2,
            StatusColor::Warning => 2,
            StatusColor::Critical => 3,
        };
        scheme_color(scheme, intensity, false).0
    }
}

#[derive(Clone, Copy)]
pub enum StatusColor {
    Good,      // Green in mono
    Warning,   // Yellow in mono
    Critical,  // Red in mono
    Info,      // Cyan in mono
    Muted,     // DarkGrey in mono
}
