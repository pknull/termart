use crate::terminal::Terminal;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::queue;
use std::io::{self, stdout, Write};

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
    term.set(start_x as i32, start_y as i32, '┌', Some(border_color), false);
    for x in 1..box_width - 1 {
        term.set((start_x + x) as i32, start_y as i32, '─', Some(border_color), false);
    }
    term.set((start_x + box_width - 1) as i32, start_y as i32, '┐', Some(border_color), false);

    // Draw content rows with side borders
    for (i, line) in lines.iter().enumerate() {
        let y = start_y + 1 + i;
        term.set(start_x as i32, y as i32, '│', Some(border_color), false);

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
    queue!(out, MoveTo(start_x as u16, start_y as u16), SetForegroundColor(border_color), Print('┌'))?;
    for x in 1..box_width - 1 {
        queue!(out, MoveTo((start_x + x) as u16, start_y as u16), Print('─'))?;
    }
    queue!(out, MoveTo((start_x + box_width - 1) as u16, start_y as u16), Print('┐'))?;

    // Content rows
    for (i, line) in lines.iter().enumerate() {
        let y = start_y + 1 + i;
        queue!(out, MoveTo(start_x as u16, y as u16), SetForegroundColor(border_color), Print('│'))?;

        let padding = max_width.saturating_sub(line.chars().count());
        let padded = format!(" {}{} ", line, " ".repeat(padding));
        queue!(out, SetForegroundColor(text_color))?;
        queue!(out, MoveTo((start_x + 1) as u16, y as u16), Print(padded))?;

        queue!(out, SetForegroundColor(border_color))?;
        queue!(out, MoveTo((start_x + box_width - 1) as u16, y as u16), Print('│'))?;
    }

    // Bottom border
    let bottom_y = start_y + box_height - 1;
    queue!(out, MoveTo(start_x as u16, bottom_y as u16), SetForegroundColor(border_color), Print('└'))?;
    for x in 1..box_width - 1 {
        queue!(out, MoveTo((start_x + x) as u16, bottom_y as u16), Print('─'))?;
    }
    queue!(out, MoveTo((start_x + box_width - 1) as u16, bottom_y as u16), Print('┘'))?;

    queue!(out, SetAttribute(Attribute::Reset), ResetColor)?;
    out.flush()?;
    Ok(())
}
