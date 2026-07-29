use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{poll, read, Event, KeyCode, KeyEventKind, KeyModifiers},
    queue,
    style::{
        force_color_output, Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io::{self, stdout, BufWriter, Write};
use std::time::Duration;

fn normalize_key(code: KeyCode, mods: KeyModifiers) -> KeyCode {
    if !mods.contains(KeyModifiers::SHIFT) {
        return code;
    }

    // Depending on the terminal keyboard protocol, crossterm may report a
    // shifted key either as the resulting character (`!`) or as the base key
    // (`1`) plus SHIFT. Normalize the latter so shared key handling works in
    // both modes.
    match code {
        KeyCode::Char('0') => KeyCode::Char(')'),
        KeyCode::Char('1') => KeyCode::Char('!'),
        KeyCode::Char('2') => KeyCode::Char('@'),
        KeyCode::Char('3') => KeyCode::Char('#'),
        KeyCode::Char('4') => KeyCode::Char('$'),
        KeyCode::Char('5') => KeyCode::Char('%'),
        KeyCode::Char('6') => KeyCode::Char('^'),
        KeyCode::Char('7') => KeyCode::Char('&'),
        KeyCode::Char('8') => KeyCode::Char('*'),
        KeyCode::Char('9') => KeyCode::Char('('),
        KeyCode::Char('/') => KeyCode::Char('?'),
        KeyCode::Char('=') => KeyCode::Char('+'),
        KeyCode::Char('-') => KeyCode::Char('_'),
        _ => code,
    }
}

fn is_key_action(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn enable_visual_colors() {
    // Color is part of the rendered content in termart, not optional CLI
    // decoration. Some launchers export NO_COLOR for their own output;
    // overriding it here keeps color schemes functional in the visualizer.
    force_color_output(true);
}

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
    pub bg: Option<Color>,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
        }
    }
}

impl Terminal {
    /// Initialize the terminal for drawing
    pub fn new(alternate_screen: bool) -> io::Result<Self> {
        enable_visual_colors();

        let (width, height) = size()?;

        if alternate_screen {
            enable_raw_mode()?;
            let mut stdout = stdout();
            if let Err(error) = (|| -> io::Result<()> {
                queue!(stdout, EnterAlternateScreen, Hide)?;
                stdout.flush()
            })() {
                let _ = queue!(stdout, Show, LeaveAlternateScreen);
                let _ = stdout.flush();
                let _ = disable_raw_mode();
                return Err(error);
            }
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
            self.back_buffer[y as usize][x as usize] = Cell {
                ch: printable_cell_char(ch),
                fg,
                bg: None,
                bold,
            };
        }
    }

    /// Set a character with both foreground and background color
    pub fn set_with_bg(
        &mut self,
        x: i32,
        y: i32,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
        bold: bool,
    ) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            self.back_buffer[y as usize][x as usize] = Cell {
                ch: printable_cell_char(ch),
                fg,
                bg,
                bold,
            };
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
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Color> = None;
        let mut last_bold = false;
        let mut has_bg = false;

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
                        last_fg = None; // Reset clears colors too
                        last_bg = None;
                    }
                    last_bold = back.bold;
                }

                // Handle foreground color changes
                if back.fg != last_fg {
                    if let Some(color) = back.fg {
                        queue!(stdout, SetForegroundColor(color))?;
                    } else {
                        queue!(stdout, ResetColor)?;
                        last_bg = None; // ResetColor clears both
                    }
                    last_fg = back.fg;
                }

                // Handle background color changes
                if back.bg != last_bg {
                    if let Some(color) = back.bg {
                        queue!(stdout, SetBackgroundColor(color))?;
                        has_bg = true;
                    } else if has_bg {
                        // Only reset bg when transitioning from bg to no-bg
                        queue!(stdout, SetBackgroundColor(Color::Reset))?;
                        has_bg = false;
                    }
                    last_bg = back.bg;
                }

                queue!(stdout, Print(back.ch))?;

                // Update front buffer
                self.front_buffer[y][x] = back.clone();
            }
        }

        // Reset attributes at end of frame
        if last_bold || last_fg.is_some() || has_bg {
            queue!(stdout, SetAttribute(Attribute::Reset), ResetColor)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// Render the entire back buffer to screen (full redraw, single flush)
    pub fn render(&mut self) -> io::Result<()> {
        let mut stdout = BufWriter::with_capacity(32 * 1024, stdout());
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Color> = None;
        let mut last_bold = false;
        let mut has_bg = false;

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
                        last_fg = None;
                        last_bg = None;
                    }
                    last_bold = cell.bold;
                }

                // Handle foreground color
                if cell.fg != last_fg {
                    if let Some(color) = cell.fg {
                        queue!(stdout, SetForegroundColor(color))?;
                    } else {
                        queue!(stdout, ResetColor)?;
                        last_bg = None;
                    }
                    last_fg = cell.fg;
                }

                // Handle background color
                if cell.bg != last_bg {
                    if let Some(color) = cell.bg {
                        queue!(stdout, SetBackgroundColor(color))?;
                        has_bg = true;
                    } else if has_bg {
                        queue!(stdout, SetBackgroundColor(Color::Reset))?;
                        has_bg = false;
                    }
                    last_bg = cell.bg;
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
                if is_key_action(key_event.kind) {
                    let code = normalize_key(key_event.code, key_event.modifiers);
                    return Ok(Some((code, key_event.modifiers)));
                }
            }
        }
        Ok(None)
    }

    /// Wait for a keypress with timeout
    pub fn wait_key(&self, timeout_ms: u64) -> io::Result<Option<KeyCode>> {
        if poll(Duration::from_millis(timeout_ms))? {
            if let Event::Key(key_event) = read()? {
                if is_key_action(key_event.kind) {
                    let code = normalize_key(key_event.code, key_event.modifiers);
                    return Ok(Some(code));
                }
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
                if cell.ch == ' ' && cell.bg.is_none() {
                    let _ = write!(out, " ");
                    continue;
                }

                if cell.bold {
                    let _ = write!(out, "\x1b[1m");
                }

                if let Some(color) = cell.fg {
                    let _ = write_ansi_fg(&mut out, color);
                }

                if let Some(color) = cell.bg {
                    let _ = write_ansi_bg(&mut out, color);
                }

                let _ = write!(out, "{}\x1b[0m", cell.ch);
            }
            let _ = writeln!(out);
        }
        let _ = out.flush();
    }
}

fn printable_cell_char(ch: char) -> char {
    if ch.is_control() {
        '�'
    } else {
        ch
    }
}

fn write_ansi_fg(out: &mut impl Write, color: Color) -> io::Result<()> {
    match color {
        Color::Rgb { r, g, b } => write!(out, "\x1b[38;2;{};{};{}m", r, g, b),
        Color::AnsiValue(v) => write!(out, "\x1b[38;5;{}m", v),
        Color::Black => write!(out, "\x1b[30m"),
        Color::DarkRed => write!(out, "\x1b[31m"),
        Color::DarkGreen => write!(out, "\x1b[32m"),
        Color::DarkYellow => write!(out, "\x1b[33m"),
        Color::DarkBlue => write!(out, "\x1b[34m"),
        Color::DarkMagenta => write!(out, "\x1b[35m"),
        Color::DarkCyan => write!(out, "\x1b[36m"),
        Color::Grey => write!(out, "\x1b[37m"),
        Color::DarkGrey => write!(out, "\x1b[90m"),
        Color::Red => write!(out, "\x1b[91m"),
        Color::Green => write!(out, "\x1b[92m"),
        Color::Yellow => write!(out, "\x1b[93m"),
        Color::Blue => write!(out, "\x1b[94m"),
        Color::Magenta => write!(out, "\x1b[95m"),
        Color::Cyan => write!(out, "\x1b[96m"),
        Color::White => write!(out, "\x1b[97m"),
        _ => Ok(()),
    }
}

fn write_ansi_bg(out: &mut impl Write, color: Color) -> io::Result<()> {
    match color {
        Color::Rgb { r, g, b } => write!(out, "\x1b[48;2;{};{};{}m", r, g, b),
        Color::AnsiValue(v) => write!(out, "\x1b[48;5;{}m", v),
        Color::Black => write!(out, "\x1b[40m"),
        Color::DarkRed => write!(out, "\x1b[41m"),
        Color::DarkGreen => write!(out, "\x1b[42m"),
        Color::DarkYellow => write!(out, "\x1b[43m"),
        Color::DarkBlue => write!(out, "\x1b[44m"),
        Color::DarkMagenta => write!(out, "\x1b[45m"),
        Color::DarkCyan => write!(out, "\x1b[46m"),
        Color::Grey => write!(out, "\x1b[47m"),
        Color::DarkGrey => write!(out, "\x1b[100m"),
        Color::Red => write!(out, "\x1b[101m"),
        Color::Green => write!(out, "\x1b[102m"),
        Color::Yellow => write!(out, "\x1b[103m"),
        Color::Blue => write!(out, "\x1b[104m"),
        Color::Magenta => write!(out, "\x1b[105m"),
        Color::Cyan => write!(out, "\x1b[106m"),
        Color::White => write!(out, "\x1b[107m"),
        _ => Ok(()),
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

#[cfg(test)]
mod tests {
    use super::{enable_visual_colors, is_key_action, normalize_key, Cell, Terminal};
    use crossterm::{
        event::{KeyCode, KeyEventKind, KeyModifiers},
        style::{force_color_output, Color, SetForegroundColor},
    };

    #[test]
    fn shifted_digits_are_normalized_to_color_shortcuts() {
        let expected = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];

        for (digit, symbol) in ('0'..='9').zip(expected) {
            assert_eq!(
                normalize_key(KeyCode::Char(digit), KeyModifiers::SHIFT),
                KeyCode::Char(symbol)
            );
        }
    }

    #[test]
    fn already_shifted_and_unmodified_keys_are_preserved() {
        assert_eq!(
            normalize_key(KeyCode::Char('!'), KeyModifiers::SHIFT),
            KeyCode::Char('!')
        );
        assert_eq!(
            normalize_key(KeyCode::Char('1'), KeyModifiers::NONE),
            KeyCode::Char('1')
        );
        assert_eq!(
            normalize_key(KeyCode::Char('/'), KeyModifiers::SHIFT),
            KeyCode::Char('?')
        );
        assert_eq!(
            normalize_key(KeyCode::Char('='), KeyModifiers::SHIFT),
            KeyCode::Char('+')
        );
    }

    #[test]
    fn key_actions_ignore_release_events() {
        assert!(is_key_action(KeyEventKind::Press));
        assert!(is_key_action(KeyEventKind::Repeat));
        assert!(!is_key_action(KeyEventKind::Release));
    }

    #[test]
    fn visual_colors_override_inherited_no_color_setting() {
        force_color_output(false);
        assert_eq!(SetForegroundColor(Color::Red).to_string(), "\u{1b}[m");

        enable_visual_colors();
        assert_eq!(SetForegroundColor(Color::Red).to_string(), "\u{1b}[38;5;9m");
    }

    #[test]
    fn terminal_cells_replace_control_characters() {
        let cells = vec![vec![Cell::default(); 3]];
        let mut terminal = Terminal {
            width: 3,
            height: 1,
            front_buffer: cells.clone(),
            back_buffer: cells,
            alternate_screen: false,
        };

        terminal.set_str(0, 0, "\u{1b}\nA", None, false);

        assert_eq!(terminal.back_buffer[0][0].ch, '�');
        assert_eq!(terminal.back_buffer[0][1].ch, '�');
        assert_eq!(terminal.back_buffer[0][2].ch, 'A');
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
