pub mod cpu;
pub mod disk;
pub mod diskio;
pub mod docker;
pub mod gpu;
pub mod layout;
pub mod mem;
pub mod net;
pub mod ps;

use crate::colors::ColorState;
use crossterm::event::{KeyCode, KeyModifiers};

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
    min_speed: f32,
}

impl MonitorState {
    pub fn new(initial_speed: f32, min_speed: f32) -> Self {
        Self {
            speed: initial_speed.max(min_speed),
            paused: false,
            colors: ColorState::new(7), // Default to mono (semantic colors)
            min_speed,
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
                let multiplier = match n {
                    0 => 10.0,
                    1 => 1.0,
                    2 => 1.25,
                    3 => 1.5,
                    4 => 1.75,
                    5 => 2.0,
                    6 => 2.5,
                    7 => 3.0,
                    8 => 4.0,
                    9 => 6.0,
                    _ => 1.0,
                };
                self.speed = self.min_speed * multiplier;
            }
            _ => {}
        }
        false
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

#[cfg(test)]
mod tests {
    use super::MonitorState;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn monitor_state_speed_presets() {
        let mut state = MonitorState::new(1.0, 0.5);
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!((state.speed - 0.5).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('9'), KeyModifiers::NONE);
        assert!((state.speed - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn monitor_state_enforces_sampling_floor() {
        let mut state = MonitorState::new(0.1, 1.0);
        assert!((state.speed - 1.0).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!((state.speed - 1.0).abs() < f32::EPSILON);
    }
}
