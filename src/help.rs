use crate::terminal::Terminal;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use std::io::{self, stdout, Write};

const SEPARATOR_WIDTH: usize = 23;

/// One keyboard control shown in an in-application help overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpEntry {
    pub keys: &'static str,
    pub action: &'static str,
}

impl HelpEntry {
    pub const fn new(keys: &'static str, action: &'static str) -> Self {
        Self { keys, action }
    }
}

/// Reusable sets of controls shared by related terminal tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalControls {
    /// Quit and help only.
    Basic,
    /// Color selection plus the basic controls.
    Colored,
    /// Pause and color selection, without numeric speed presets.
    Pausable,
    /// Numeric speed presets and color selection, without pause.
    SpeedControlled,
    /// Pause, speed, color selection, quit, and help.
    Animated,
}

const BASIC_CONTROLS: &[HelpEntry] = &[
    HelpEntry::new("q/Esc", "Quit"),
    HelpEntry::new("?", "Toggle help"),
];

const COLORED_CONTROLS: &[HelpEntry] = &[
    HelpEntry::new("!-()", "Color scheme"),
    HelpEntry::new("q/Esc", "Quit"),
    HelpEntry::new("?", "Toggle help"),
];

const ANIMATED_CONTROLS: &[HelpEntry] = &[
    HelpEntry::new("Space", "Pause/resume"),
    HelpEntry::new("1-9", "Speed (1=fast)"),
    HelpEntry::new("!-()", "Color scheme"),
    HelpEntry::new("q/Esc", "Quit"),
    HelpEntry::new("?", "Toggle help"),
];

const PAUSABLE_CONTROLS: &[HelpEntry] = &[
    HelpEntry::new("Space", "Pause/resume"),
    HelpEntry::new("!-()", "Color scheme"),
    HelpEntry::new("q/Esc", "Quit"),
    HelpEntry::new("?", "Toggle help"),
];

const SPEED_CONTROLS: &[HelpEntry] = &[
    HelpEntry::new("1-9", "Speed (1=fast)"),
    HelpEntry::new("!-()", "Color scheme"),
    HelpEntry::new("q/Esc", "Quit"),
    HelpEntry::new("?", "Toggle help"),
];

/// Structured help content for one terminal tool.
///
/// Keeping labels separate from presentation prevents individual tools from
/// inventing their own separators, alignment, and global-control wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpSpec {
    pub title: &'static str,
    pub controls: &'static [HelpEntry],
    pub global_controls: GlobalControls,
}

impl HelpSpec {
    pub const fn new(
        title: &'static str,
        controls: &'static [HelpEntry],
        global_controls: GlobalControls,
    ) -> Self {
        Self {
            title,
            controls,
            global_controls,
        }
    }

    pub const fn basic(title: &'static str, controls: &'static [HelpEntry]) -> Self {
        Self::new(title, controls, GlobalControls::Basic)
    }

    pub const fn colored(title: &'static str, controls: &'static [HelpEntry]) -> Self {
        Self::new(title, controls, GlobalControls::Colored)
    }

    pub const fn animated(title: &'static str, controls: &'static [HelpEntry]) -> Self {
        Self::new(title, controls, GlobalControls::Animated)
    }

    pub const fn pausable(title: &'static str, controls: &'static [HelpEntry]) -> Self {
        Self::new(title, controls, GlobalControls::Pausable)
    }

    pub const fn speed_controlled(title: &'static str, controls: &'static [HelpEntry]) -> Self {
        Self::new(title, controls, GlobalControls::SpeedControlled)
    }

    pub fn render(&self) -> String {
        let global = match self.global_controls {
            GlobalControls::Basic => BASIC_CONTROLS,
            GlobalControls::Colored => COLORED_CONTROLS,
            GlobalControls::Pausable => PAUSABLE_CONTROLS,
            GlobalControls::SpeedControlled => SPEED_CONTROLS,
            GlobalControls::Animated => ANIMATED_CONTROLS,
        };

        let key_width = self
            .controls
            .iter()
            .chain(global)
            .map(|entry| entry.keys.chars().count())
            .max()
            .unwrap_or(0);
        let local_lines = format_entries(self.controls, key_width);
        let global_lines = format_entries(global, key_width);
        let content_width = std::iter::once(self.title)
            .chain((!self.controls.is_empty()).then_some("GLOBAL CONTROLS"))
            .map(str::chars)
            .map(Iterator::count)
            .chain(
                local_lines
                    .iter()
                    .chain(&global_lines)
                    .map(|line| line.chars().count()),
            )
            .max()
            .unwrap_or(0)
            .max(SEPARATOR_WIDTH);
        let separator = "─".repeat(content_width);
        let mut lines = vec![self.title.to_string(), separator.clone()];

        lines.extend(local_lines);
        if !self.controls.is_empty() {
            lines.push(separator.clone());
            lines.push("GLOBAL CONTROLS".to_string());
        }
        lines.extend(global_lines);
        lines.push(separator);
        lines.join("\n")
    }
}

fn format_entries(entries: &[HelpEntry], key_width: usize) -> Vec<String> {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{:<width$}  {}",
                entry.keys,
                entry.action,
                width = key_width
            )
        })
        .collect()
}

/// Render a centered help overlay box with the provided text.
pub fn render_help_overlay(term: &mut Terminal, width: u16, height: u16, help_text: &str) {
    if help_text.is_empty() {
        return;
    }

    let lines: Vec<&str> = help_text.lines().collect();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let box_width = max_width + 4; // 2 chars padding each side
    let box_height = lines.len() + 2; // 1 row padding top/bottom

    // Center the box
    let start_x = (width as usize).saturating_sub(box_width) / 2;
    let start_y = (height as usize).saturating_sub(box_height) / 2;

    let border_color = Color::White;
    let text_color = Color::Grey;

    // Draw top border: ┌─────┐
    term.set(
        start_x as i32,
        start_y as i32,
        '┌',
        Some(border_color),
        false,
    );
    for x in 1..box_width - 1 {
        term.set(
            (start_x + x) as i32,
            start_y as i32,
            '─',
            Some(border_color),
            false,
        );
    }
    term.set(
        (start_x + box_width - 1) as i32,
        start_y as i32,
        '┐',
        Some(border_color),
        false,
    );

    // Draw content rows with side borders
    for (i, line) in lines.iter().enumerate() {
        let y = start_y + 1 + i;
        term.set(start_x as i32, y as i32, '│', Some(border_color), false);

        let padding = max_width.saturating_sub(line.chars().count());
        let padded = format!(" {}{} ", line, " ".repeat(padding));
        for (j, ch) in padded.chars().enumerate() {
            term.set(
                (start_x + 1 + j) as i32,
                y as i32,
                ch,
                Some(text_color),
                false,
            );
        }

        term.set(
            (start_x + box_width - 1) as i32,
            y as i32,
            '│',
            Some(border_color),
            false,
        );
    }

    // Draw bottom border: └─────┘
    let bottom_y = start_y + box_height - 1;
    term.set(
        start_x as i32,
        bottom_y as i32,
        '└',
        Some(border_color),
        false,
    );
    for x in 1..box_width - 1 {
        term.set(
            (start_x + x) as i32,
            bottom_y as i32,
            '─',
            Some(border_color),
            false,
        );
    }
    term.set(
        (start_x + box_width - 1) as i32,
        bottom_y as i32,
        '┘',
        Some(border_color),
        false,
    );
}

/// Render structured help using the common overlay presentation.
pub fn render_help_spec(term: &mut Terminal, width: u16, height: u16, spec: &HelpSpec) {
    render_help_overlay(term, width, height, &spec.render());
}

/// Show a modal help overlay without modifying the back buffer.
/// Returns true if the user requested quit (q/Esc) while the overlay is open.
pub fn show_help_modal(term: &mut Terminal, help_text: &str) -> io::Result<bool> {
    if help_text.is_empty() {
        return Ok(false);
    }

    let (width, height) = term.size();
    render_help_overlay_direct(width, height, help_text)?;

    loop {
        if let Some(code) = term.wait_key(50)? {
            match code {
                KeyCode::Char('?') => break,
                KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
                _ => {}
            }
        }
    }

    // Restore previous frame from back buffer.
    term.render()?;
    Ok(false)
}

/// Show structured help as a modal overlay.
pub fn show_help_spec_modal(term: &mut Terminal, spec: &HelpSpec) -> io::Result<bool> {
    show_help_modal(term, &spec.render())
}

fn render_help_overlay_direct(width: u16, height: u16, help_text: &str) -> io::Result<()> {
    let lines: Vec<&str> = help_text.lines().collect();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let box_width = max_width + 4;
    let box_height = lines.len() + 2;

    let start_x = (width as usize).saturating_sub(box_width) / 2;
    let start_y = (height as usize).saturating_sub(box_height) / 2;

    let border_color = Color::White;
    let text_color = Color::Grey;

    let mut out = stdout();

    // Top border
    queue!(
        out,
        MoveTo(start_x as u16, start_y as u16),
        SetForegroundColor(border_color),
        Print('┌')
    )?;
    for x in 1..box_width - 1 {
        queue!(
            out,
            MoveTo((start_x + x) as u16, start_y as u16),
            Print('─')
        )?;
    }
    queue!(
        out,
        MoveTo((start_x + box_width - 1) as u16, start_y as u16),
        Print('┐')
    )?;

    // Content rows
    for (i, line) in lines.iter().enumerate() {
        let y = start_y + 1 + i;
        queue!(
            out,
            MoveTo(start_x as u16, y as u16),
            SetForegroundColor(border_color),
            Print('│')
        )?;

        let padding = max_width.saturating_sub(line.chars().count());
        let padded = format!(" {}{} ", line, " ".repeat(padding));
        queue!(out, SetForegroundColor(text_color))?;
        queue!(out, MoveTo((start_x + 1) as u16, y as u16), Print(padded))?;

        queue!(out, SetForegroundColor(border_color))?;
        queue!(
            out,
            MoveTo((start_x + box_width - 1) as u16, y as u16),
            Print('│')
        )?;
    }

    // Bottom border
    let bottom_y = start_y + box_height - 1;
    queue!(
        out,
        MoveTo(start_x as u16, bottom_y as u16),
        SetForegroundColor(border_color),
        Print('└')
    )?;
    for x in 1..box_width - 1 {
        queue!(
            out,
            MoveTo((start_x + x) as u16, bottom_y as u16),
            Print('─')
        )?;
    }
    queue!(
        out,
        MoveTo((start_x + box_width - 1) as u16, bottom_y as u16),
        Print('┘')
    )?;

    queue!(out, SetAttribute(Attribute::Reset), ResetColor)?;
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HelpEntry, HelpSpec};

    #[test]
    fn structured_help_aligns_controls_and_adds_shared_controls() {
        const HELP: HelpSpec = HelpSpec::animated(
            "TEST TOOL",
            &[
                HelpEntry::new("x", "Short key"),
                HelpEntry::new("Long", "Long key"),
            ],
        );
        let help = HELP.render();

        assert!(help.starts_with("TEST TOOL\n"));
        assert!(help.contains("x      Short key"));
        assert!(help.contains("Long   Long key"));
        assert!(help.contains("GLOBAL CONTROLS"));
        assert!(help.contains("Space  Pause/resume"));
        assert!(help.contains("?      Toggle help"));
    }

    #[test]
    fn basic_help_omits_empty_local_section() {
        let help = HelpSpec::basic("STATIC TOOL", &[]).render();

        assert!(!help.contains("GLOBAL CONTROLS"));
        assert!(help.contains("q/Esc  Quit"));
        assert!(help.contains("?      Toggle help"));
    }
}
