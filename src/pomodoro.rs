use crate::colors::{ColorState, scheme_color};
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
pub enum PomodoroPhase {
    Work,
    ShortBreak,
    LongBreak,
}

impl PomodoroPhase {
    fn color(&self) -> Color {
        match self {
            PomodoroPhase::Work => Color::Red,
            PomodoroPhase::ShortBreak => Color::Green,
            PomodoroPhase::LongBreak => Color::Blue,
        }
    }

    fn intensity(&self) -> u8 {
        match self {
            PomodoroPhase::Work => 3,        // Brightest - active work
            PomodoroPhase::ShortBreak => 2,  // Medium - short rest
            PomodoroPhase::LongBreak => 1,   // Dimmer - long rest
        }
    }
}

pub struct PomodoroConfig {
    pub work_mins: u32,
    pub short_break_mins: u32,
    pub long_break_mins: u32,
    pub pomodoros_until_long: u32,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_mins: 25,
            short_break_mins: 5,
            long_break_mins: 15,
            pomodoros_until_long: 4,
        }
    }
}

struct PomodoroState {
    phase: PomodoroPhase,
    remaining_secs: u32,
    total_secs: u32,
    pomodoros_completed: u32,
    paused: bool,
    last_tick: Instant,
    bell_rung: bool,
    flash_frame: usize,
}

impl PomodoroState {
    fn new(config: &PomodoroConfig) -> Self {
        let total_secs = config.work_mins * 60;
        Self {
            phase: PomodoroPhase::Work,
            remaining_secs: total_secs,
            total_secs,
            pomodoros_completed: 0,
            paused: false,
            last_tick: Instant::now(),
            bell_rung: false,
            flash_frame: 0,
        }
    }

    fn progress(&self) -> f32 {
        1.0 - (self.remaining_secs as f32 / self.total_secs as f32)
    }

    fn tick(&mut self) {
        if self.paused || self.remaining_secs == 0 {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_tick) >= Duration::from_secs(1) {
            self.remaining_secs = self.remaining_secs.saturating_sub(1);
            self.last_tick = now;
        }
    }

    fn next_phase(&mut self, config: &PomodoroConfig) {
        match self.phase {
            PomodoroPhase::Work => {
                self.pomodoros_completed += 1;
                if self.pomodoros_completed % config.pomodoros_until_long == 0 {
                    self.phase = PomodoroPhase::LongBreak;
                    self.total_secs = config.long_break_mins * 60;
                } else {
                    self.phase = PomodoroPhase::ShortBreak;
                    self.total_secs = config.short_break_mins * 60;
                }
            }
            PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => {
                self.phase = PomodoroPhase::Work;
                self.total_secs = config.work_mins * 60;
            }
        }
        self.remaining_secs = self.total_secs;
        self.last_tick = Instant::now();
        self.bell_rung = false;
    }

    fn reset(&mut self, config: &PomodoroConfig) {
        self.phase = PomodoroPhase::Work;
        self.total_secs = config.work_mins * 60;
        self.remaining_secs = self.total_secs;
        self.pomodoros_completed = 0;
        self.paused = false;
        self.last_tick = Instant::now();
        self.bell_rung = false;
        self.flash_frame = 0;
    }
}

// Compact 3-line digits
const DIGITS: [[&str; 3]; 10] = [
    ["█▀█", "█ █", "▀▀▀"],  // 0
    [" ▀█", "  █", "  ▀"],  // 1
    ["▀▀█", "█▀▀", "▀▀▀"],  // 2
    ["▀▀█", " ▀█", "▀▀▀"],  // 3
    ["█ █", "▀▀█", "  ▀"],  // 4
    ["█▀▀", "▀▀█", "▀▀▀"],  // 5
    ["█▀▀", "█▀█", "▀▀▀"],  // 6
    ["▀▀█", "  █", "  ▀"],  // 7
    ["█▀█", "█▀█", "▀▀▀"],  // 8
    ["█▀█", "▀▀█", "▀▀▀"],  // 9
];

const COLON: [&str; 3] = ["   ", " ● ", " ● "];

// Compact tomato (7 lines)
fn draw_tomato(term: &mut Terminal, cx: usize, cy: usize, progress: f32, color: Color) {
    let tomato = [
        r"    \|/    ",
        r"  .-'`-.   ",
        r" /       \ ",
        r"|         |",
        r"|         |",
        r" \       / ",
        r"  `'---'`  ",
    ];

    let fill_rows = 4; // rows 2-5 can be filled
    let filled = (progress * fill_rows as f32).ceil() as usize;

    let start_x = cx.saturating_sub(tomato[0].len() / 2);

    for (i, line) in tomato.iter().enumerate() {
        let y = cy + i;
        let is_fillable = (2..=5).contains(&i);
        let fill_index = if is_fillable { 5 - i } else { 0 }; // Fill from bottom up
        let should_fill = is_fillable && fill_index < filled;

        // Draw outline
        for (j, ch) in line.chars().enumerate() {
            let x = start_x + j;
            if ch == ' ' { continue; }

            let c = if i == 0 { Color::Green } else { Color::DarkRed };
            term.set(x as i32, y as i32, ch, Some(c), false);
        }

        // Fill interior
        if should_fill {
            let (left, right) = match i {
                2 => (2, 9),
                3 => (1, 10),
                4 => (1, 10),
                5 => (2, 9),
                _ => (0, 0),
            };
            for j in left..right {
                term.set((start_x + j) as i32, y as i32, '█', Some(color), false);
            }
        }
    }
}

fn draw_big_time(term: &mut Terminal, cx: usize, cy: usize, mins: u32, secs: u32, color: Color) {
    let time_str = format!("{:02}:{:02}", mins, secs);
    let digit_width = 3;
    let colon_width = 3;
    let spacing = 1;

    let total_width = 4 * digit_width + colon_width + 4 * spacing;
    let start_x = cx.saturating_sub(total_width / 2);

    let mut x_pos = start_x;

    for ch in time_str.chars() {
        let pattern: &[&str; 3] = if ch == ':' {
            &COLON
        } else {
            let digit = ch.to_digit(10).unwrap_or(0) as usize;
            &DIGITS[digit]
        };

        let width = if ch == ':' { colon_width } else { digit_width };

        for (row, line) in pattern.iter().enumerate() {
            let y = cy + row;
            for (col, pch) in line.chars().enumerate() {
                if pch != ' ' {
                    term.set((x_pos + col) as i32, y as i32, pch, Some(color), false);
                }
            }
        }

        x_pos += width + spacing;
    }
}

fn draw_progress_bar(term: &mut Terminal, x: usize, y: usize, width: usize, progress: f32, color: Color) {
    const METER_CHAR: char = '■';
    let filled = (progress * width as f32) as usize;

    for i in 0..width {
        let c = if i < filled { color } else { Color::DarkGrey };
        term.set((x + i) as i32, y as i32, METER_CHAR, Some(c), false);
    }
}

fn draw_pomodoro_dots(term: &mut Terminal, cx: usize, y: usize, completed: u32, until_long: u32, colors: &ColorState) {
    let total = until_long;
    let dot_spacing = 3;
    let total_width = (total as usize - 1) * dot_spacing + total as usize;
    let start_x = cx.saturating_sub(total_width / 2);

    for i in 0..total {
        let x = start_x + i as usize * (dot_spacing + 1);
        let in_current_cycle = completed % until_long;
        let (ch, color) = if i < in_current_cycle {
            let c = if colors.is_mono() {
                Color::Red
            } else {
                scheme_color(colors.scheme, 3, true).0
            };
            ('●', c)
        } else {
            ('○', Color::DarkGrey)
        };
        term.set(x as i32, y as i32, ch, Some(color), false);
    }
}

pub fn run(config: PomodoroConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = PomodoroState::new(&config);
    let mut colors = ColorState::new(7); // Default to mono (semantic colors)

    loop {
        // Handle input
        if let Ok(Some((code, _mods))) = term.check_key() {
            if !colors.handle_key(code) {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char(' ') => state.paused = !state.paused,
                    KeyCode::Char('s') => state.next_phase(&config),
                    KeyCode::Char('r') => state.reset(&config),
                    KeyCode::Enter => {
                        if state.remaining_secs == 0 {
                            state.next_phase(&config);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle resize
        if let Ok((new_w, new_h)) = size() {
            let (cur_w, cur_h) = term.size();
            if new_w != cur_w || new_h != cur_h {
                term.resize(new_w, new_h);
                term.clear_screen()?;
            }
        }

        // Update timer
        state.tick();

        // Ring bell when timer first hits zero
        if state.remaining_secs == 0 && !state.bell_rung {
            print!("\x07"); // Terminal bell
            state.bell_rung = true;
        }

        // Update flash frame
        state.flash_frame = state.flash_frame.wrapping_add(1);

        // Render
        term.clear();
        let (w, h) = term.size();
        let w = w as usize;
        let h = h as usize;

        let cx = w / 2;

        // Determine display color: gray when paused, flash when done, otherwise phase color
        let done = state.remaining_secs == 0;
        let flash_on = done && (state.flash_frame / 3) % 2 == 0;
        let phase_color = if state.paused {
            Color::DarkGrey
        } else if flash_on {
            Color::White
        } else if colors.is_mono() {
            state.phase.color()
        } else {
            scheme_color(colors.scheme, state.phase.intensity(), true).0
        };

        // Calculate vertical center for compact layout (total ~12 lines)
        let content_height = 12;
        let start_y = h.saturating_sub(content_height) / 2;
        let mut y = start_y;

        // Tomato (7 lines)
        draw_tomato(&mut term, cx, y, state.progress(), phase_color);
        y += 8;

        // Time display (3 lines)
        let mins = state.remaining_secs / 60;
        let secs = state.remaining_secs % 60;
        draw_big_time(&mut term, cx, y, mins, secs, phase_color);
        y += 4;

        // Progress bar
        let bar_width = 30.min(w - 4);
        let bar_x = cx.saturating_sub(bar_width / 2);
        draw_progress_bar(&mut term, bar_x, y, bar_width, state.progress(), phase_color);
        y += 1;

        // Pomodoro dots
        draw_pomodoro_dots(&mut term, cx, y, state.pomodoros_completed, config.pomodoros_until_long, &colors);

        term.present()?;
        term.sleep(0.1);
    }

    Ok(())
}
