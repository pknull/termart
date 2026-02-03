pub mod layout;
pub mod cpu;
pub mod mem;
pub mod disk;
pub mod diskio;
pub mod net;
pub mod gpu;
pub mod ps;
pub mod docker;

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
}

/// Global help section appended to all monitor help text
pub const MONITOR_GLOBAL_HELP: &str = "\
───────────────────────
 GLOBAL CONTROLS
 Space   Pause/resume
 1-9     Speed (1=fast)
 !-()    Color scheme
 q/Esc   Quit
 ?       Close help
───────────────────────";

/// Build monitor help text from a title and optional extra lines
pub fn build_help(title: &str, extra: &str) -> String {
    if extra.is_empty() {
        format!("{title}\n─────────────────\n{MONITOR_GLOBAL_HELP}")
    } else {
        format!("{title}\n─────────────────\n{extra}\n{MONITOR_GLOBAL_HELP}")
    }
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
    use super::{build_help, MONITOR_GLOBAL_HELP, MonitorState};
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn build_help_without_extra() {
        let text = build_help("CPU MONITOR", "");
        assert!(text.starts_with("CPU MONITOR"));
        assert!(text.contains(MONITOR_GLOBAL_HELP));
    }

    #[test]
    fn build_help_with_extra() {
        let text = build_help("PROCESS LIST", "m  Cycle sort");
        assert!(text.contains("PROCESS LIST"));
        assert!(text.contains("m  Cycle sort"));
        assert!(text.contains(MONITOR_GLOBAL_HELP));
    }

    #[test]
    fn monitor_state_speed_presets() {
        let mut state = MonitorState::new(1.0);
        state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert!((state.speed - 0.1).abs() < f32::EPSILON);
        state.handle_key(KeyCode::Char('9'), KeyModifiers::NONE);
        assert!((state.speed - 3.0).abs() < f32::EPSILON);
    }
}
