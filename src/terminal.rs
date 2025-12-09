use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{poll, read, Event, KeyCode},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Attribute, SetAttribute},
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen, size,
    },
};
use std::io::{self, Write, stdout};
use std::time::Duration;

/// Terminal abstraction for rendering
pub struct Terminal {
    width: u16,
    height: u16,
    buffer: Vec<Vec<Cell>>,
    alternate_screen: bool,
}

/// A single cell in the terminal buffer
#[derive(Clone)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bold: false,
        }
    }
}

impl Terminal {
    /// Initialize the terminal for drawing
    pub fn new(alternate_screen: bool) -> io::Result<Self> {
        let (width, height) = size()?;

        if alternate_screen {
            enable_raw_mode()?;
            execute!(stdout(), EnterAlternateScreen, Hide)?;
        }

        let buffer = vec![vec![Cell::default(); width as usize]; height as usize];

        Ok(Self {
            width,
            height,
            buffer,
            alternate_screen,
        })
    }

    /// Get terminal dimensions
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        for row in &mut self.buffer {
            for cell in row {
                *cell = Cell::default();
            }
        }
    }

    /// Clear the actual terminal
    pub fn clear_screen(&self) -> io::Result<()> {
        execute!(stdout(), Clear(ClearType::All))?;
        Ok(())
    }

    /// Set a character at position with optional color
    pub fn set(&mut self, x: i32, y: i32, ch: char, fg: Option<Color>, bold: bool) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            self.buffer[y as usize][x as usize] = Cell { ch, fg, bold };
        }
    }

    /// Set a string starting at position
    pub fn set_str(&mut self, x: i32, y: i32, s: &str, fg: Option<Color>, bold: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.set(x + i as i32, y, ch, fg, bold);
        }
    }

    /// Draw a single cell immediately (for live mode)
    pub fn draw_cell(&self, x: i32, y: i32, ch: char, fg: Option<Color>, bold: bool) -> io::Result<()> {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            let mut stdout = stdout();
            execute!(stdout, MoveTo(x as u16, y as u16))?;

            if bold {
                execute!(stdout, SetAttribute(Attribute::Bold))?;
            }

            if let Some(color) = fg {
                execute!(stdout, SetForegroundColor(color), Print(ch), ResetColor)?;
            } else {
                execute!(stdout, Print(ch))?;
            }

            if bold {
                execute!(stdout, SetAttribute(Attribute::Reset))?;
            }

            stdout.flush()?;
        }
        Ok(())
    }

    /// Render the entire buffer to screen
    pub fn render(&self) -> io::Result<()> {
        let mut stdout = stdout();
        execute!(stdout, MoveTo(0, 0))?;

        for (y, row) in self.buffer.iter().enumerate() {
            execute!(stdout, MoveTo(0, y as u16))?;

            for cell in row {
                if cell.bold {
                    execute!(stdout, SetAttribute(Attribute::Bold))?;
                }

                if let Some(color) = cell.fg {
                    execute!(stdout, SetForegroundColor(color), Print(cell.ch), ResetColor)?;
                } else {
                    execute!(stdout, Print(cell.ch))?;
                }

                if cell.bold {
                    execute!(stdout, SetAttribute(Attribute::Reset))?;
                }
            }
        }

        stdout.flush()?;
        Ok(())
    }

    /// Check for keypress (non-blocking), returns (code, modifiers)
    pub fn check_key(&self) -> io::Result<Option<(KeyCode, crossterm::event::KeyModifiers)>> {
        if poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = read()? {
                return Ok(Some((key_event.code, key_event.modifiers)));
            }
        }
        Ok(None)
    }

    /// Wait for a keypress with timeout
    pub fn wait_key(&self, timeout_ms: u64) -> io::Result<Option<KeyCode>> {
        if poll(Duration::from_millis(timeout_ms))? {
            if let Event::Key(key_event) = read()? {
                return Ok(Some(key_event.code));
            }
        }
        Ok(None)
    }

    /// Sleep for specified duration
    pub fn sleep(&self, seconds: f32) {
        std::thread::sleep(Duration::from_secs_f32(seconds));
    }

    /// Print buffer to stdout with ANSI colors (for print mode)
    pub fn print_to_stdout(&self) {
        for row in &self.buffer {
            for cell in row {
                if cell.ch == ' ' {
                    print!(" ");
                    continue;
                }

                if cell.bold {
                    print!("\x1b[1m");
                }

                if let Some(color) = cell.fg {
                    match color {
                        Color::Rgb { r, g, b } => {
                            print!("\x1b[38;2;{};{};{}m", r, g, b);
                        }
                        Color::AnsiValue(v) => {
                            print!("\x1b[38;5;{}m", v);
                        }
                        // Standard colors (0-7)
                        Color::Black => print!("\x1b[30m"),
                        Color::DarkRed => print!("\x1b[31m"),
                        Color::DarkGreen => print!("\x1b[32m"),
                        Color::DarkYellow => print!("\x1b[33m"),
                        Color::DarkBlue => print!("\x1b[34m"),
                        Color::DarkMagenta => print!("\x1b[35m"),
                        Color::DarkCyan => print!("\x1b[36m"),
                        Color::Grey => print!("\x1b[37m"),
                        // Bright colors (8-15)
                        Color::DarkGrey => print!("\x1b[90m"),
                        Color::Red => print!("\x1b[91m"),
                        Color::Green => print!("\x1b[92m"),
                        Color::Yellow => print!("\x1b[93m"),
                        Color::Blue => print!("\x1b[94m"),
                        Color::Magenta => print!("\x1b[95m"),
                        Color::Cyan => print!("\x1b[96m"),
                        Color::White => print!("\x1b[97m"),
                        _ => {}
                    }
                }

                print!("{}", cell.ch);
                print!("\x1b[0m");
            }
            println!();
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.alternate_screen {
            let _ = execute!(stdout(), Show, LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }
}

/// Helper to create RGB colors
pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb { r, g, b }
}

/// Predefined colors for bonsai (using standard terminal colors)
pub mod colors {
    use crossterm::style::Color;

    // Wood colors - use yellow/dark yellow for brown-like appearance
    pub const WOOD_DARK: Color = Color::DarkYellow;
    pub const WOOD_LIGHT: Color = Color::Yellow;

    // Leaf colors - use green shades
    pub const LEAF_DARK: Color = Color::DarkGreen;
    pub const LEAF_LIGHT: Color = Color::Green;

    // Pot colors
    pub const POT: Color = Color::DarkYellow;
    pub const POT_DARK: Color = Color::DarkRed;

    // Fractal colors
    pub const FRACTAL_PRIMARY: Color = Color::Cyan;
    pub const FRACTAL_SECONDARY: Color = Color::Magenta;
    pub const FRACTAL_TERTIARY: Color = Color::Yellow;
}
