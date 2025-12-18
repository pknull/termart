pub mod layout;
pub mod cpu;
pub mod mem;
pub mod disk;
pub mod diskio;
pub mod net;
pub mod gpu;
pub mod ps;
pub mod docker;

use crate::colors::{ColorState, scheme_color};
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Color;

#[derive(Clone, Copy, PartialEq)]
pub enum MonitorType {
    Cpu,
    Mem,
    Disk,
    Io,
    Net,
    Gpu,
}

#[derive(Clone)]
pub struct MonitorConfig {
    pub monitor_type: MonitorType,
    pub time_step: f32,
    #[allow(dead_code)]
    pub debug: bool,
}

pub struct MonitorState {
    pub speed: f32,
    pub paused: bool,
    pub colors: ColorState,
}

impl MonitorState {
    pub fn new(initial_speed: f32) -> Self {
        Self {
            speed: initial_speed,
            paused: false,
            colors: ColorState::new(7), // Default to mono (semantic colors)
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> bool {
        // Check color keys first
        if self.colors.handle_key(code) {
            return false;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char(' ') => self.paused = !self.paused,
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as u8;
                self.speed = match n {
                    0 => 2.0,
                    1 => 0.1,
                    2 => 0.2,
                    3 => 0.3,
                    4 => 0.5,
                    5 => 0.7,
                    6 => 1.0,
                    7 => 1.5,
                    8 => 2.0,
                    9 => 3.0,
                    _ => self.speed,
                };
            }
            _ => {}
        }
        false
    }

    /// Get color based on value percentage (0-100) - semantic in mono, scheme otherwise
    pub fn value_color(&self, pct: f32) -> Color {
        if self.colors.is_mono() {
            // Semantic colors: green < 50%, yellow 50-80%, red > 80%
            if pct < 50.0 {
                Color::Green
            } else if pct < 80.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        } else {
            let intensity = if pct < 30.0 { 1 } else if pct < 60.0 { 2 } else { 3 };
            scheme_color(self.colors.scheme, intensity, pct > 80.0).0
        }
    }

    /// Get bar color for progress bars
    pub fn bar_color(&self, pct: f32) -> Color {
        self.value_color(pct)
    }

    /// Get text/label color
    pub fn text_color(&self) -> Color {
        if self.colors.is_mono() {
            Color::White
        } else {
            scheme_color(self.colors.scheme, 2, false).0
        }
    }

    /// Get muted/secondary text color
    pub fn muted_color(&self) -> Color {
        if self.colors.is_mono() {
            Color::DarkGrey
        } else {
            scheme_color(self.colors.scheme, 0, false).0
        }
    }

    /// Get header/title color
    pub fn header_color(&self) -> Color {
        if self.colors.is_mono() {
            Color::Cyan
        } else {
            scheme_color(self.colors.scheme, 3, true).0
        }
    }
}

pub fn run(config: MonitorConfig) -> std::io::Result<()> {
    match config.monitor_type {
        MonitorType::Cpu => cpu::run(config),
        MonitorType::Mem => mem::run(config),
        MonitorType::Disk => disk::run(config),
        MonitorType::Io => diskio::run(config),
        MonitorType::Net => net::run(config),
        MonitorType::Gpu => gpu::run(config),
    }
}

