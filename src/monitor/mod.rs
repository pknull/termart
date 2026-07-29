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
use crate::help::{render_help_overlay, HelpSpec};
use crate::terminal::Terminal;
use crossterm::event::{KeyCode, KeyModifiers};
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const MAX_COLLECTOR_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_STATUS_ERROR_CHARS: usize = 256;

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
    pub show_help: bool,
    min_speed: f32,
    default_speed: f32,
    last_sample: Option<Instant>,
    last_attempt: Option<Instant>,
    sample_error: Option<String>,
    feedback: Option<(String, Instant)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorAction {
    None,
    Quit,
    SampleNow,
}

impl MonitorState {
    pub fn new(initial_speed: f32, min_speed: f32) -> Self {
        let speed = initial_speed.max(min_speed);
        Self {
            speed,
            paused: false,
            colors: ColorState::new(7), // Default to mono (semantic colors)
            show_help: false,
            min_speed,
            default_speed: speed,
            last_sample: None,
            last_attempt: None,
            sample_error: None,
            feedback: None,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> MonitorAction {
        // Check color keys first
        if self.colors.handle_key(code, modifiers) {
            self.set_feedback(format!("Color: {}", self.colors.name()));
            return MonitorAction::None;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => return MonitorAction::Quit,
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                self.set_feedback(if self.paused { "Paused" } else { "Live" });
            }
            KeyCode::Char('r') => {
                self.set_feedback("Refreshed");
                return MonitorAction::SampleNow;
            }
            KeyCode::Char('.') => {
                if self.paused {
                    self.set_feedback("Single sample");
                    return MonitorAction::SampleNow;
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.speed = (self.speed / 1.25).max(self.min_speed);
                self.interval_feedback();
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let max_speed = self.default_speed.max(self.min_speed * 10.0);
                self.speed = (self.speed * 1.25).min(max_speed);
                self.interval_feedback();
            }
            KeyCode::Char('d') => {
                self.speed = self.default_speed;
                self.interval_feedback();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let Some(n) = c.to_digit(10) else {
                    return MonitorAction::None;
                };
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
                self.interval_feedback();
            }
            _ => {}
        }
        MonitorAction::None
    }

    pub fn should_sample(&self, action: MonitorAction) -> bool {
        action == MonitorAction::SampleNow
            || (!self.paused
                && self
                    .last_attempt
                    .is_none_or(|sample| sample.elapsed().as_secs_f32() >= self.speed))
    }

    pub fn poll_delay(&self) -> f32 {
        0.05
    }

    pub fn mark_sampled(&mut self) {
        let now = Instant::now();
        self.last_sample = Some(now);
        self.last_attempt = Some(now);
        self.sample_error = None;
    }

    pub fn mark_sample_failed(&mut self, error: impl std::fmt::Display) {
        self.last_attempt = Some(Instant::now());
        self.sample_error = Some(truncate_message(&error.to_string(), MAX_STATUS_ERROR_CHARS));
    }

    pub fn record_sample(&mut self, result: std::io::Result<()>) -> bool {
        match result {
            Ok(()) => {
                self.mark_sampled();
                true
            }
            Err(error) => {
                self.mark_sample_failed(error);
                false
            }
        }
    }

    pub fn set_feedback(&mut self, message: impl Into<String>) {
        self.feedback = Some((message.into(), Instant::now()));
    }

    pub fn render_help(&self, term: &mut Terminal, width: u16, height: u16, spec: &HelpSpec) {
        if !self.show_help || width == 0 || height == 0 {
            return;
        }

        render_help_overlay(term, width, height, &self.help_text(spec, width as usize));
    }

    fn help_text(&self, spec: &HelpSpec, width: usize) -> String {
        const RUNTIME_PREFIX: &str = "RUNTIME  ";
        let runtime_width = width.saturating_sub(4 + RUNTIME_PREFIX.chars().count());
        let rendered = spec.render();
        let mut lines = rendered.lines();
        let title = lines.next().unwrap_or(spec.title);
        let separator = lines.next().unwrap_or_default();
        let controls = lines.collect::<Vec<_>>().join("\n");

        format!(
            "{title}\n{separator}\n{RUNTIME_PREFIX}{}\n{separator}\n{controls}",
            self.status_text(runtime_width).trim_start()
        )
    }

    fn status_text(&self, width: usize) -> String {
        let mode = if self.sample_error.is_some() {
            "ERROR"
        } else if self.paused {
            "PAUSED"
        } else {
            "LIVE"
        };
        let sample_age = match self.last_sample {
            Some(sample) if sample.elapsed() < Duration::from_secs(1) => "sample now".to_string(),
            Some(sample) => format!("sample {}s ago", sample.elapsed().as_secs()),
            None => "awaiting sample".to_string(),
        };
        let mut fields = vec![mode.to_string()];
        if let Some(error) = &self.sample_error {
            fields.push(error.clone());
            if self.paused {
                fields.push("PAUSED".to_string());
            }
        }
        fields.push(format_interval(self.speed));
        fields.push(self.colors.name().to_string());
        fields.push(sample_age);
        if let Some((message, shown_at)) = &self.feedback {
            if shown_at.elapsed() < Duration::from_millis(1750) {
                fields.push(message.clone());
            }
        }

        let status = format!(" {}", fields.join(" │ "));
        status.chars().take(width).collect()
    }

    fn interval_feedback(&mut self) {
        self.set_feedback(format!("Interval: {}", format_interval(self.speed)));
    }
}

pub fn command_output_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn()?;
    let process_group = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("failed to capture command stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("failed to capture command stderr"))?;

    let stdout_reader = spawn_reader(stdout);
    let stderr_reader = spawn_reader(stderr);
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;

    loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(child_status) => status = child_status,
                Err(error) => {
                    terminate_process_group(&mut child, process_group);
                    return Err(error);
                }
            }
        }
        if stdout.is_none() {
            stdout = poll_reader(&stdout_reader);
        }
        if stderr.is_none() {
            stderr = poll_reader(&stderr_reader);
        }

        if status.is_some() && stdout.is_some() && stderr.is_some() {
            let child_status = match status.take() {
                Some(status) => status,
                None => unreachable!("status checked above"),
            };
            let stdout_result = match stdout.take() {
                Some(stdout) => stdout,
                None => unreachable!("stdout checked above"),
            };
            let stderr_result = match stderr.take() {
                Some(stderr) => stderr,
                None => unreachable!("stderr checked above"),
            };
            return Ok(Output {
                status: child_status,
                stdout: stdout_result?,
                stderr: stderr_result?,
            });
        }

        if Instant::now() >= deadline {
            terminate_process_group(&mut child, process_group);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("command timed out after {:.1}s", timeout.as_secs_f32()),
            ));
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_reader(reader: impl Read + Send + 'static) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(read_bounded(reader));
    });
    receiver
}

fn poll_reader(receiver: &Receiver<io::Result<Vec<u8>>>) -> Option<io::Result<Vec<u8>>> {
    match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => {
            Some(Err(io::Error::other("command output reader stopped")))
        }
    }
}

fn read_bounded(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut exceeded_limit = false;

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }

        let remaining = MAX_COLLECTOR_OUTPUT_BYTES.saturating_sub(retained.len());
        let keep = count.min(remaining);
        retained.extend_from_slice(&buffer[..keep]);
        exceeded_limit |= keep < count;
    }

    if exceeded_limit {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "collector output exceeded 1 MiB",
        ))
    } else {
        Ok(retained)
    }
}

fn terminate_process_group(child: &mut std::process::Child, process_group: u32) {
    #[cfg(unix)]
    {
        // The collector is its own process-group leader. Killing the group also
        // closes pipes inherited by helper descendants.
        unsafe {
            libc::kill(-(process_group as i32), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

pub(crate) fn truncate_message(message: &str, max_chars: usize) -> String {
    if message.chars().count() <= max_chars {
        message.to_string()
    } else if max_chars == 0 {
        String::new()
    } else {
        let mut truncated: String = message.chars().take(max_chars - 1).collect();
        truncated.push('…');
        truncated
    }
}

fn format_interval(seconds: f32) -> String {
    if seconds < 1.0 {
        format!("{:.0}ms", seconds * 1000.0)
    } else if seconds < 10.0 {
        format!("{seconds:.1}s")
    } else {
        format!("{seconds:.0}s")
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
    use super::{
        command_output_with_timeout, format_interval, read_bounded, truncate_message,
        MonitorAction, MonitorState, MAX_COLLECTOR_OUTPUT_BYTES,
    };
    use crate::help::HelpSpec;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::io::Cursor;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn monitor_state_speed_presets() {
        let mut state = MonitorState::new(1.0, 0.5);
        assert_eq!(
            state.handle_key(KeyCode::Char('1'), KeyModifiers::NONE),
            MonitorAction::None
        );
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

    #[test]
    fn monitor_actions_support_pause_refresh_step_and_help() {
        let mut state = MonitorState::new(1.0, 0.5);

        assert_eq!(
            state.handle_key(KeyCode::Char('.'), KeyModifiers::NONE),
            MonitorAction::None
        );
        state.handle_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(state.paused);
        assert!(!state.should_sample(MonitorAction::None));
        assert_eq!(
            state.handle_key(KeyCode::Char('.'), KeyModifiers::NONE),
            MonitorAction::SampleNow
        );
        assert!(state.should_sample(MonitorAction::SampleNow));
        assert_eq!(
            state.handle_key(KeyCode::Char('r'), KeyModifiers::NONE),
            MonitorAction::SampleNow
        );

        state.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(state.show_help);
        assert_eq!(
            state.handle_key(KeyCode::Esc, KeyModifiers::NONE),
            MonitorAction::Quit
        );
    }

    #[test]
    fn scheduled_sampling_is_decoupled_from_input_polling() {
        let mut state = MonitorState::new(1.0, 0.5);
        assert!(state.should_sample(MonitorAction::None));

        state.mark_sampled();
        assert!(!state.should_sample(MonitorAction::None));
        state.last_attempt = Some(Instant::now() - Duration::from_millis(1100));
        assert!(state.should_sample(MonitorAction::None));
        assert!((state.poll_delay() - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn fine_interval_controls_clamp_and_restore_default() {
        let mut state = MonitorState::new(2.0, 0.5);

        for _ in 0..20 {
            state.handle_key(KeyCode::Char('+'), KeyModifiers::NONE);
        }
        assert!((state.speed - 0.5).abs() < f32::EPSILON);

        for _ in 0..40 {
            state.handle_key(KeyCode::Char('-'), KeyModifiers::NONE);
        }
        assert!((state.speed - 5.0).abs() < f32::EPSILON);

        state.handle_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!((state.speed - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn interval_labels_use_readable_units() {
        assert_eq!(format_interval(0.5), "500ms");
        assert_eq!(format_interval(1.0), "1.0s");
        assert_eq!(format_interval(12.0), "12s");
    }

    #[test]
    fn help_text_exposes_runtime_state_and_feedback() {
        let mut state = MonitorState::new(1.0, 0.5);
        let help = HelpSpec::monitor("MONITOR", &[]);
        let initial = state.help_text(&help, 100);
        assert!(initial.contains("RUNTIME"));
        assert!(initial.contains("LIVE"));
        assert!(initial.contains("1.0s"));
        assert!(initial.contains("Mono"));
        assert!(initial.contains("awaiting sample"));
        assert!(initial.find("RUNTIME").unwrap() < initial.find("Space").unwrap());

        state.handle_key(KeyCode::Char(' '), KeyModifiers::NONE);
        state.mark_sampled();
        let paused = state.help_text(&help, 100);
        assert!(paused.contains("PAUSED"));
        assert!(paused.contains("sample now"));
        assert!(paused.contains("Paused"));
        assert_eq!(state.status_text(8), " PAUSED ");
    }

    #[test]
    fn sample_failures_are_visible_and_rate_limited() {
        let mut state = MonitorState::new(1.0, 0.5);
        state.mark_sample_failed("collector unavailable");

        let status = state.status_text(100);
        assert!(status.contains("ERROR"));
        assert!(status.contains("collector unavailable"));
        assert!(!state.should_sample(MonitorAction::None));

        state.mark_sampled();
        assert!(!state.status_text(100).contains("collector unavailable"));
    }

    #[test]
    fn paused_sample_failures_keep_the_error_visible_first() {
        let mut state = MonitorState::new(1.0, 0.5);
        state.paused = true;
        state.mark_sample_failed("collector unavailable");

        assert_eq!(state.status_text(30), " ERROR │ collector unavailable");
    }

    #[test]
    fn external_collectors_are_killed_after_their_timeout() {
        let started = Instant::now();
        let error = command_output_with_timeout(
            Command::new("sh").args(["-c", "sleep 2 &"]),
            Duration::from_millis(30),
        )
        .expect_err("collector descendant holding output pipes should time out");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn collector_output_and_status_errors_are_bounded() {
        let oversized = vec![b'x'; MAX_COLLECTOR_OUTPUT_BYTES + 1];
        let error = read_bounded(Cursor::new(oversized)).expect_err("output must be capped");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);

        let summary = truncate_message(&"λ".repeat(300), 256);
        assert_eq!(summary.chars().count(), 256);
        assert!(summary.ends_with('…'));
    }
}
