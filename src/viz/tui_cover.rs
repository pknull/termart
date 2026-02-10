use crate::colors::ColorState;
use crate::config::FractalConfig;
use crate::help::render_help_overlay;
use crate::settings::Settings;
use crate::terminal::Terminal;
use crate::tui::cover::{calc_cover_dimensions, render_cover_halfblock, render_cover_halfblock_palette, resized_rgba, CoverArtLoader, CoverRenderCache};
use crate::tui::mpris_client::MprisClient;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::io;
use std::time::{Duration, Instant};

const HELP_TEXT: &str = "\
TUI COVER
─────────────────
r      Reconnect
&      Full RGB cover
?      Close help
───────────────────────
 GLOBAL CONTROLS
 !-()   Color scheme
 q/Esc  Quit
───────────────────────";

pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut colors = ColorState::new(7);
    let mut show_help = false;

    let settings = Settings::load();
    let mut mpris = MprisClient::new(settings.tui.players.clone());
    let mut cover_loader = CoverArtLoader::new();
    let mut render_cache: Option<CoverRenderCache> = None;

    mpris.connect().ok();
    let mut state = mpris.get_state();

    let tick_rate = Duration::from_secs_f32(config.time_step.max(0.02));
    let state_update_rate = Duration::from_millis(500);
    let mut last_state_update = Instant::now();

    let (mut prev_w, mut prev_h) = term.size();

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
            if let Event::Key(key) = event::read()? {
                let code = normalize_key(key.code, key.modifiers);
                if !colors.handle_key(code) {
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('?') => show_help = !show_help,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            mpris.connect().ok();
                            state = mpris.get_state();
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_state_update.elapsed() >= state_update_rate {
            state = mpris.get_state();
            last_state_update = Instant::now();
        }

        term.clear();

        let (width, height) = term.size();
        if !state.connected {
            render_centered_text(term, width, height, "No MPRIS-compatible player found.\nPress 'r' to reconnect.");
        } else if let Some(ref url) = state.art_url {
            cover_loader.request(url);
            if let Some(img) = cover_loader.get(url) {
                let (art_w, art_h_cells, x_off, _y_off) = calc_cover_dimensions(width, height);
                if art_w > 0 && art_h_cells > 0 {
                    let pixel_h = art_h_cells * 2;
                    let rgba = resized_rgba(&mut render_cache, url, art_w, pixel_h, img);
                    if colors.scheme == 7 {
                        render_cover_halfblock(term, rgba, x_off, 0, art_w, art_h_cells);
                    } else {
                        render_cover_halfblock_palette(term, rgba, x_off, 0, art_w, art_h_cells, colors.scheme);
                    }
                }
            } else {
                render_centered_text(term, width, height, "Loading...");
            }
        } else {
            render_centered_text(term, width, height, "[No Cover]");
        }

        if show_help {
            render_help_overlay(term, width, height, HELP_TEXT);
        }

        term.present()?;
        term.sleep(tick_rate.as_secs_f32());
    }

    Ok(())
}

fn normalize_key(code: KeyCode, mods: KeyModifiers) -> KeyCode {
    if code == KeyCode::Char('/') && mods.contains(KeyModifiers::SHIFT) {
        KeyCode::Char('?')
    } else {
        code
    }
}

fn render_centered_text(term: &mut Terminal, width: u16, height: u16, text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let start_x = (width as usize).saturating_sub(max_width) / 2;
    let start_y = (height as usize).saturating_sub(lines.len()) / 2;

    for (i, line) in lines.iter().enumerate() {
        term.set_str(start_x as i32, (start_y + i) as i32, line, None, false);
    }
}
