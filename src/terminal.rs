use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{poll, read, Event, KeyCode},
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor, Attribute, SetAttribute},
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen, size,
    },
};
use std::io::{self, Write, stdout, BufWriter};
use std::time::Duration;

/// Terminal abstraction for rendering
pub struct Terminal {
    width: u16,
    height: u16,
    front_buffer: Vec<Vec<Cell>>,
    back_buffer: Vec<Vec<Cell>>,
    alternate_screen: bool,
}

/// A single cell in the terminal buffer
#[derive(Clone, PartialEq)]
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
            let mut stdout = stdout();
            queue!(stdout, EnterAlternateScreen, Hide)?;
            stdout.flush()?;
        }

        let front_buffer = vec![vec![Cell::default(); width as usize]; height as usize];
        let back_buffer = vec![vec![Cell::default(); width as usize]; height as usize];

        Ok(Self {
            width,
            height,
            front_buffer,
            back_buffer,
            alternate_screen,
        })
    }

    /// Get terminal dimensions
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Resize buffers to match new terminal size
    pub fn resize(&mut self, width: u16, height: u16) {
        if width != self.width || height != self.height {
            self.width = width;
            self.height = height;
            self.front_buffer = vec![vec![Cell::default(); width as usize]; height as usize];
            self.back_buffer = vec![vec![Cell::default(); width as usize]; height as usize];
        }
    }

    /// Clear the back buffer
    pub fn clear(&mut self) {
        for row in &mut self.back_buffer {
            for cell in row {
                *cell = Cell::default();
            }
        }
    }

    /// Clear the actual terminal and both buffers
    pub fn clear_screen(&mut self) -> io::Result<()> {
        let mut stdout = stdout();
        queue!(stdout, Clear(ClearType::All))?;
        stdout.flush()?;
        // Reset both buffers to force full redraw
        for row in &mut self.front_buffer {
            for cell in row {
                *cell = Cell::default();
            }
        }
        for row in &mut self.back_buffer {
            for cell in row {
                *cell = Cell::default();
            }
        }
        Ok(())
    }

    /// Set a character in the back buffer
    pub fn set(&mut self, x: i32, y: i32, ch: char, fg: Option<Color>, bold: bool) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            self.back_buffer[y as usize][x as usize] = Cell { ch, fg, bold };
        }
    }

    /// Set a string starting at position in the back buffer
    pub fn set_str(&mut self, x: i32, y: i32, s: &str, fg: Option<Color>, bold: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.set(x + i as i32, y, ch, fg, bold);
        }
    }

    /// Render only changed cells (differential update) with single flush
    pub fn present(&mut self) -> io::Result<()> {
        let mut stdout = BufWriter::with_capacity(32 * 1024, stdout());
        let mut last_color: Option<Color> = None;
        let mut last_bold = false;

        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let back = &self.back_buffer[y][x];
                let front = &self.front_buffer[y][x];

                // Skip unchanged cells
                if back == front {
                    continue;
                }

                // Move cursor
                queue!(stdout, MoveTo(x as u16, y as u16))?;

                // Handle bold attribute changes
                if back.bold != last_bold {
                    if back.bold {
                        queue!(stdout, SetAttribute(Attribute::Bold))?;
                    } else {
                        queue!(stdout, SetAttribute(Attribute::Reset))?;
                        last_color = None; // Reset clears color too
                    }
                    last_bold = back.bold;
                }

                // Handle color changes
                if back.fg != last_color {
                    if let Some(color) = back.fg {
                        queue!(stdout, SetForegroundColor(color))?;
                    } else {
                        queue!(stdout, ResetColor)?;
                    }
                    last_color = back.fg;
                }

                queue!(stdout, Print(back.ch))?;

                // Update front buffer
                self.front_buffer[y][x] = back.clone();
            }
        }

        // Reset attributes at end of frame
        if last_bold || last_color.is_some() {
            queue!(stdout, SetAttribute(Attribute::Reset), ResetColor)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// Render the entire back buffer to screen (full redraw, single flush)
    pub fn render(&mut self) -> io::Result<()> {
        let mut stdout = BufWriter::with_capacity(32 * 1024, stdout());
        let mut last_color: Option<Color> = None;
        let mut last_bold = false;

        queue!(stdout, MoveTo(0, 0))?;

        for (y, row) in self.back_buffer.iter().enumerate() {
            queue!(stdout, MoveTo(0, y as u16))?;

            for cell in row {
                // Handle bold
                if cell.bold != last_bold {
                    if cell.bold {
                        queue!(stdout, SetAttribute(Attribute::Bold))?;
                    } else {
                        queue!(stdout, SetAttribute(Attribute::Reset))?;
                        last_color = None;
                    }
                    last_bold = cell.bold;
                }

                // Handle color
                if cell.fg != last_color {
                    if let Some(color) = cell.fg {
                        queue!(stdout, SetForegroundColor(color))?;
                    } else {
                        queue!(stdout, ResetColor)?;
                    }
                    last_color = cell.fg;
                }

                queue!(stdout, Print(cell.ch))?;
            }
        }

        queue!(stdout, SetAttribute(Attribute::Reset), ResetColor)?;
        stdout.flush()?;

        // Sync front buffer
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                self.front_buffer[y][x] = self.back_buffer[y][x].clone();
            }
        }

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
        let mut out = BufWriter::new(stdout());
        for row in &self.back_buffer {
            for cell in row {
                if cell.ch == ' ' {
                    let _ = write!(out, " ");
                    continue;
                }

                if cell.bold {
                    let _ = write!(out, "\x1b[1m");
                }

                if let Some(color) = cell.fg {
                    let _ = match color {
                        Color::Rgb { r, g, b } => {
                            write!(out, "\x1b[38;2;{};{};{}m", r, g, b)
                        }
                        Color::AnsiValue(v) => {
                            write!(out, "\x1b[38;5;{}m", v)
                        }
                        // Standard colors (0-7)
                        Color::Black => write!(out, "\x1b[30m"),
                        Color::DarkRed => write!(out, "\x1b[31m"),
                        Color::DarkGreen => write!(out, "\x1b[32m"),
                        Color::DarkYellow => write!(out, "\x1b[33m"),
                        Color::DarkBlue => write!(out, "\x1b[34m"),
                        Color::DarkMagenta => write!(out, "\x1b[35m"),
                        Color::DarkCyan => write!(out, "\x1b[36m"),
                        Color::Grey => write!(out, "\x1b[37m"),
                        // Bright colors (8-15)
                        Color::DarkGrey => write!(out, "\x1b[90m"),
                        Color::Red => write!(out, "\x1b[91m"),
                        Color::Green => write!(out, "\x1b[92m"),
                        Color::Yellow => write!(out, "\x1b[93m"),
                        Color::Blue => write!(out, "\x1b[94m"),
                        Color::Magenta => write!(out, "\x1b[95m"),
                        Color::Cyan => write!(out, "\x1b[96m"),
                        Color::White => write!(out, "\x1b[97m"),
                        _ => Ok(()),
                    };
                }

                let _ = write!(out, "{}\x1b[0m", cell.ch);
            }
            let _ = writeln!(out);
        }
        let _ = out.flush();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.alternate_screen {
            let mut stdout = stdout();
            let _ = queue!(stdout, Show, LeaveAlternateScreen);
            let _ = stdout.flush();
            let _ = disable_raw_mode();
        }
    }
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

    // Pot color
    pub const POT: Color = Color::DarkYellow;
}
