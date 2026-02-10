use crate::colors::{scheme_color, ColorState};
use crate::config::FractalConfig;
use crate::help::render_help_overlay;
use crate::settings::Settings;
use crate::terminal::Terminal;
use crate::tui::mpris_client::{format_duration, MprisClient};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::execute;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::borrow::Cow;
use std::io;
use std::time::{Duration, Instant};

const HELP_TEXT: &str = "\
TUI CONTROL
─────────────────
Space  Play/pause
n/p    Next/prev
h/l    Seek -/+5s
j/k    Volume -/+
r      Reconnect
Mouse  Click bar to seek
?      Close help
───────────────────────
 GLOBAL CONTROLS
 !-()   Color scheme
 q/Esc  Quit
───────────────────────";

#[derive(Copy, Clone, Default)]
struct Area {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl Area {
    fn contains(&self, col: u16, row: u16) -> bool {
        col >= self.x
            && col < self.x + self.width
            && row >= self.y
            && row < self.y + self.height
    }
}

#[derive(Default)]
struct UiAreas {
    progress: Option<Area>,
    controls: Option<Area>,
}

pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut colors = ColorState::new(7);
    let mut show_help = false;
    let settings = Settings::load();
    let keybinds = settings.tui.keybinds.clone();

    let mut mpris = MprisClient::new(settings.tui.players.clone());
    mpris.connect().ok();
    let mut state = mpris.get_state();

    let tick_rate = Duration::from_secs_f32(config.time_step.max(0.02));
    let state_update_rate = Duration::from_millis(500);
    let mut last_state_update = Instant::now();

    let (mut prev_w, mut prev_h) = term.size();
    let mut areas = UiAreas::default();

    let _mouse_guard = MouseCaptureGuard::enable()?;

    loop {
        if let Ok((w, h)) = crossterm::terminal::size() {
            if w != prev_w || h != prev_h {
                term.resize(w, h);
                term.clear_screen()?;
                prev_w = w;
                prev_h = h;
            }
        }

        if event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key) => {
                    let code = normalize_key(key.code, key.modifiers);
                    if !colors.handle_key(code) {
                        match code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('?') => show_help = !show_help,
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                mpris.connect().ok();
                                state = mpris.get_state();
                            }
                            _ => {
                                let key_str = key_to_string(code, key.modifiers);
                                if keybinds.quit.iter().any(|k| k == &key_str) {
                                    break;
                                } else if keybinds.toggle.iter().any(|k| k == &key_str) {
                                    mpris.toggle().ok();
                                    state = mpris.get_state();
                                } else if keybinds.next.iter().any(|k| k == &key_str) {
                                    mpris.next().ok();
                                    state = mpris.get_state();
                                } else if keybinds.prev.iter().any(|k| k == &key_str) {
                                    mpris.prev().ok();
                                    state = mpris.get_state();
                                } else if keybinds.seek_forward.iter().any(|k| k == &key_str) {
                                    mpris.seek_forward(Duration::from_secs(5)).ok();
                                    state = mpris.get_state();
                                } else if keybinds.seek_backward.iter().any(|k| k == &key_str) {
                                    mpris.seek_backward(Duration::from_secs(5)).ok();
                                    state = mpris.get_state();
                                } else if keybinds.volume_up.iter().any(|k| k == &key_str) {
                                    mpris.adjust_volume(0.05).ok();
                                    state = mpris.get_state();
                                } else if keybinds.volume_down.iter().any(|k| k == &key_str) {
                                    mpris.adjust_volume(-0.05).ok();
                                    state = mpris.get_state();
                                }
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        if let Some(progress) = areas.progress {
                            if progress.contains(mouse.column, mouse.row) && progress.width > 0 {
                                let rel = mouse.column.saturating_sub(progress.x) as f64;
                                let ratio = (rel / progress.width as f64).clamp(0.0, 1.0);
                                if state.length.as_secs() > 0 {
                                    let new_pos = Duration::from_secs_f64(
                                        state.length.as_secs_f64() * ratio,
                                    );
                                    mpris.set_position(new_pos).ok();
                                    state = mpris.get_state();
                                }
                            }
                        }
                        if let Some(controls) = areas.controls {
                            if controls.contains(mouse.column, mouse.row) {
                                mpris.toggle().ok();
                                state = mpris.get_state();
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if last_state_update.elapsed() >= state_update_rate {
            state = mpris.get_state();
            last_state_update = Instant::now();
        }

        term.clear();
        let (width, height) = term.size();
        areas = render_ui(term, width, height, &state, colors.scheme);

        if show_help {
            render_help_overlay(term, width, height, HELP_TEXT);
        }

        term.present()?;
        term.sleep(tick_rate.as_secs_f32());
    }

    Ok(())
}

fn render_ui(
    term: &mut Terminal,
    width: u16,
    height: u16,
    state: &crate::tui::mpris_client::PlayerState,
    scheme: u8,
) -> UiAreas {
    let (primary, primary_bold) = scheme_color(scheme, 2, true);
    let (muted, muted_bold) = scheme_color(scheme, 0, false);
    let (accent, accent_bold) = scheme_color(scheme, 3, true);

    if !state.connected {
        draw_centered_line(
            term,
            width,
            height as usize / 2,
            "No MPRIS-compatible player found. Press 'r' to reconnect.",
            muted,
            muted_bold,
        );
        return UiAreas::default();
    }

    let total_lines = 5usize;
    let start_y = (height as usize).saturating_sub(total_lines) / 2;

    // Track metadata
    draw_centered_line(term, width, start_y, &state.title, primary, primary_bold);
    draw_centered_line(term, width, start_y + 1, &state.artists, primary, primary_bold);
    draw_centered_line(term, width, start_y + 2, &state.album, muted, muted_bold);

    let mut areas = UiAreas::default();

    let progress_y = start_y + 3;
    let status_y = start_y + 4;

    if progress_y < height as usize {
        let bar_width = width.saturating_sub(4).max(10);
        let bar_width = bar_width.min(width);
        let bar_x = (width.saturating_sub(bar_width)) / 2;

        let ratio = if state.length.as_secs() > 0 {
            (state.position.as_secs_f64() / state.length.as_secs_f64()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let filled = (ratio * bar_width as f64).round() as u16;

        for i in 0..bar_width {
            let ch = if i < filled { '█' } else { '░' };
            let (color, bold) = if i < filled { (accent, accent_bold) } else { (muted, muted_bold) };
            term.set((bar_x + i) as i32, progress_y as i32, ch, Some(color), bold);
        }

        areas.progress = Some(Area {
            x: bar_x,
            y: progress_y as u16,
            width: bar_width,
            height: 1,
        });
    }

    if status_y < height as usize {
        let bar_x = (width.saturating_sub(width.saturating_sub(4).max(10).min(width))) / 2;
        let bar_end = bar_x + width.saturating_sub(4).max(10).min(width);
        let sy = status_y as i32;

        // Left: controls icons (shuffle, play/pause, repeat)
        let mut ctrl_parts: Vec<(&str, crossterm::style::Color, bool)> = Vec::new();
        if state.shuffle {
            ctrl_parts.push(("⇌ ", muted, muted_bold));
        }
        ctrl_parts.push((state.status.icon(), accent, accent_bold));
        let repeat_icon = state.loop_status.icon();
        if !repeat_icon.is_empty() {
            ctrl_parts.push((" ", muted, false));
            ctrl_parts.push((repeat_icon, muted, muted_bold));
        }

        let mut cx = bar_x as i32;
        for (text, color, bold) in &ctrl_parts {
            term.set_str(cx, sy, text, Some(*color), *bold);
            cx += text.chars().count() as i32;
        }
        let ctrl_width = (cx - bar_x as i32) as u16;
        areas.controls = Some(Area {
            x: bar_x,
            y: status_y as u16,
            width: ctrl_width,
            height: 1,
        });

        // Center: time
        let total = if state.length.as_secs() == 0 {
            "--:--".to_string()
        } else {
            format_duration(state.length)
        };
        let time_str = format!("{} / {}", format_duration(state.position), total);
        let time_x = center_x(width, time_str.chars().count());
        term.set_str(time_x as i32, sy, &time_str, Some(primary), primary_bold);

        // Right: volume
        if state.volume >= 0.0 {
            let vol_str = format!("{}%", (state.volume * 100.0).round() as u32);
            let vol_x = (bar_end as usize).saturating_sub(vol_str.chars().count());
            term.set_str(vol_x as i32, sy, &vol_str, Some(muted), muted_bold);
        }
    }

    areas
}

fn draw_centered_line(term: &mut Terminal, width: u16, y: usize, text: &str, color: crossterm::style::Color, bold: bool) {
    if y >= term.size().1 as usize {
        return;
    }
    let available = width as usize;
    let trimmed = truncate_to_width(text, available);
    let x = center_x(width, trimmed.chars().count());
    term.set_str(x as i32, y as i32, &trimmed, Some(color), bold);
}

fn center_x(width: u16, text_width: usize) -> usize {
    (width as usize).saturating_sub(text_width) / 2
}

fn truncate_to_width(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars().take(width).collect()
}

struct MouseCaptureGuard;

impl MouseCaptureGuard {
    fn enable() -> io::Result<Self> {
        let mut stdout = io::stdout();
        execute!(stdout, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCaptureGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture);
    }
}

fn normalize_key(code: KeyCode, mods: KeyModifiers) -> KeyCode {
    if code == KeyCode::Char('/') && mods.contains(KeyModifiers::SHIFT) {
        KeyCode::Char('?')
    } else {
        code
    }
}

/// Convert key event to string representation (mplay-compatible)
fn key_to_string(code: KeyCode, modifiers: KeyModifiers) -> Cow<'static, str> {
    let key_name: Cow<'static, str> = match code {
        KeyCode::Char(' ') => Cow::Borrowed(" "),
        KeyCode::Char(c) => Cow::Owned(c.to_string()),
        KeyCode::Enter => Cow::Borrowed("Enter"),
        KeyCode::Esc => Cow::Borrowed("Escape"),
        KeyCode::Tab => Cow::Borrowed("Tab"),
        KeyCode::Backspace => Cow::Borrowed("Backspace"),
        KeyCode::Delete => Cow::Borrowed("Delete"),
        KeyCode::Left => Cow::Borrowed("Left"),
        KeyCode::Right => Cow::Borrowed("Right"),
        KeyCode::Up => Cow::Borrowed("Up"),
        KeyCode::Down => Cow::Borrowed("Down"),
        KeyCode::Home => Cow::Borrowed("Home"),
        KeyCode::End => Cow::Borrowed("End"),
        KeyCode::PageUp => Cow::Borrowed("PageUp"),
        KeyCode::PageDown => Cow::Borrowed("PageDown"),
        KeyCode::F(n) => Cow::Owned(format!("F{}", n)),
        _ => Cow::Borrowed("Unknown"),
    };

    let has_ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let has_alt = modifiers.contains(KeyModifiers::ALT);
    let has_shift = modifiers.contains(KeyModifiers::SHIFT) && match code {
        KeyCode::Char(c) => !c.is_alphabetic(),
        _ => true,
    };

    if !has_ctrl && !has_alt && !has_shift {
        return key_name;
    }

    let mut result = String::with_capacity(16);
    if has_ctrl {
        result.push_str("Ctrl+");
    }
    if has_alt {
        result.push_str("Alt+");
    }
    if has_shift {
        result.push_str("Shift+");
    }
    result.push_str(&key_name);
    Cow::Owned(result)
}
