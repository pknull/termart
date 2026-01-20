//! Audio spectrum visualizer (CAVA-style)
//!
//! Captures system audio and displays real-time frequency spectrum bars.
//! Bars are mirrored horizontally from the center line (growing both up and down)
//! with configurable spacing between bars.
//!
//! # Algorithm
//! Uses FFT analysis with logarithmic frequency-to-bar mapping
//! (more bars allocated to bass frequencies for musical visualization).
//! Bar heights are smoothed with exponential attack/decay for visual appeal.
//!
//! # Keybindings
//! - Left/Right: Adjust bar count (±8 bars)
//! - 1-9, 0: Adjust animation speed
//! - Shift+0-9: Change color scheme (Shift+8 for rainbow)
//! - Space: Pause
//! - q/Esc: Quit

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use super::{scheme_color, VizState};
use std::io;
use std::sync::{Arc, Mutex};
use std::os::unix::io::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::fs::File;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use crossterm::style::Color;

/// Configuration constants for the audio visualizer
mod constants {
    /// FFT buffer size - must be power of 2 for efficient FFT computation
    pub const BUFFER_SIZE: usize = 2048;

    /// Default number of frequency bars to display
    pub const DEFAULT_BAR_COUNT: usize = 64;
    /// Minimum allowed bar count (prevents overly sparse display)
    pub const MIN_BARS: usize = 8;
    /// Maximum allowed bar count (prevents performance issues)
    pub const MAX_BARS: usize = 200;
    /// Step size when adjusting bar count with arrow keys
    pub const BAR_ADJUST_STEP: usize = 8;

    /// Decay rate for bar fall animation (0.0-1.0, higher = slower fall)
    /// 0.85 provides smooth visual decay similar to CAVA
    pub const DECAY_RATE: f32 = 0.85;
    /// Attack rate for bar rise animation (0.0-1.0, higher = faster rise)
    /// 0.7 provides responsive but smooth rise
    pub const ATTACK_RATE: f32 = 0.7;

    /// Base decay time for brightest tier (tier 3)
    pub const TIER_DECAY_BASE: f32 = 0.04;
    /// Additional time per tier level (tier 2 = base + step, tier 1 = base + 2*step, etc.)
    pub const TIER_DECAY_STEP: f32 = 0.03;
    /// Number of color tiers (3=brightest, 0=dimmest, then collapse)
    pub const NUM_TIERS: i8 = 4;
    /// Visual height decay multiplier (height decays over total tier time * this factor)
    pub const VISUAL_HEIGHT_DECAY_FACTOR: f32 = 1.2;

    /// Minimum frequency for spectrum analysis (Hz) - below human hearing threshold
    pub const FREQ_MIN_HZ: f32 = 20.0;
    /// Maximum frequency for spectrum analysis (Hz) - upper limit of typical music content
    pub const FREQ_MAX_HZ: f32 = 16000.0;
    /// Amplitude sensitivity multiplier - scales FFT output to screen height
    pub const SENSITIVITY: f32 = 150.0;
    /// Ratio of bar height where "peak" coloring begins (brighter/bolder)
    pub const PEAK_THRESHOLD: f32 = 0.8;

    /// Sleep interval when displaying errors (seconds)
    pub const ERROR_POLL_INTERVAL: f32 = 0.1;
    /// Minimum frame time to cap at ~60fps (seconds)
    pub const MIN_FRAME_TIME: f32 = 0.016;

    /// Minimum gap between bars (in characters)
    pub const BAR_GAP: usize = 1;

    /// Color scheme index for rainbow mode
    pub const RAINBOW_SCHEME: u8 = 8;

    /// Debug log file path (in /tmp for easy access)
    pub const DEBUG_LOG_PATH: &str = "/tmp/termart-audio.log";
    /// Debug log file permissions (owner read/write only - 0o600)
    pub const DEBUG_LOG_MODE: u32 = 0o600;
}

use constants::*;

/// Get rainbow color based on position (0.0 to 1.0 across the spectrum)
fn rainbow_color(pos: f32) -> Color {
    // HSV to RGB with H cycling 0-360, S=1, V=1
    let h = (pos * 360.0) % 360.0;
    let c = 1.0_f32;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::Rgb {
        r: (r * 255.0) as u8,
        g: (g * 255.0) as u8,
        b: (b * 255.0) as u8,
    }
}

/// RAII guard to suppress stderr during ALSA device enumeration
/// Restores stderr when dropped
struct StderrSuppressor {
    saved_fd: i32,
    dev_null: File,
}

impl StderrSuppressor {
    fn new() -> Option<Self> {
        // Open /dev/null
        let dev_null = File::open("/dev/null").ok()?;

        // Save current stderr fd
        let saved_fd = unsafe { libc::dup(2) };
        if saved_fd < 0 {
            return None;
        }

        // Redirect stderr to /dev/null - check for errors
        let dup2_result = unsafe { libc::dup2(dev_null.as_raw_fd(), 2) };
        if dup2_result < 0 {
            // dup2 failed - close saved_fd and return None
            unsafe { libc::close(saved_fd); }
            return None;
        }

        Some(Self { saved_fd, dev_null })
    }
}

impl Drop for StderrSuppressor {
    fn drop(&mut self) {
        // Restore original stderr
        unsafe {
            libc::dup2(self.saved_fd, 2);
            libc::close(self.saved_fd);
        }
        let _ = &self.dev_null; // Keep dev_null alive until here
    }
}

fn suppress_alsa_errors() -> Option<StderrSuppressor> {
    StderrSuppressor::new()
}

/// Validate that a PulseAudio source name contains only safe characters.
/// Valid names: alphanumeric, dots, dashes, underscores, colons, at-signs
/// (e.g., "alsa_output.pci-0000_03_00.1.hdmi-stereo.monitor", "@DEFAULT_SINK@")
fn is_valid_source_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '@')
    })
}

/// Shared audio buffer between capture thread and render thread
/// Stereo audio buffer storing left and right channels separately
struct StereoAudioBuffer {
    left: Vec<f32>,
    right: Vec<f32>,
    write_pos: usize,
}

impl StereoAudioBuffer {
    fn new() -> Self {
        Self {
            left: vec![0.0; BUFFER_SIZE],
            right: vec![0.0; BUFFER_SIZE],
            write_pos: 0,
        }
    }

    /// Push interleaved stereo samples (L, R, L, R, ...)
    fn push_stereo_samples(&mut self, interleaved: &[f32]) {
        for chunk in interleaved.chunks(2) {
            if chunk.len() == 2 {
                self.left[self.write_pos] = chunk[0];
                self.right[self.write_pos] = chunk[1];
                self.write_pos = (self.write_pos + 1) % BUFFER_SIZE;
            }
        }
    }

    /// Push mono samples to both channels
    fn push_mono_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.left[self.write_pos] = sample;
            self.right[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % BUFFER_SIZE;
        }
    }

    /// Copy samples into preallocated buffers (oldest to newest order)
    fn copy_samples(&self, left: &mut [f32], right: &mut [f32]) {
        for i in 0..BUFFER_SIZE {
            let idx = (self.write_pos + i) % BUFFER_SIZE;
            left[i] = self.left[idx];
            right[i] = self.right[idx];
        }
    }
}

/// Get decay time for a specific tier (higher tiers = shorter time, lower tiers = longer time)
/// Tier 3: 0.04s, Tier 2: 0.07s, Tier 1: 0.10s, Tier 0: 0.13s
fn decay_time_for_tier(tier: i8) -> f32 {
    let tiers_from_brightest = (NUM_TIERS - 1 - tier).max(0) as f32;
    TIER_DECAY_BASE + tiers_from_brightest * TIER_DECAY_STEP
}

/// Per-level decay state for a single bar
/// Each height level tracks its own color tier and decay timer independently
/// Tiers: 3 (brightest) → 2 → 1 → 0 (dimmest) → -1 (collapsed)
struct BarDecayState {
    /// Current color tier for each height level (-1 = collapsed, 0-3 = active)
    level_tiers: Vec<i8>,
    /// Time remaining in current tier for each level (seconds)
    level_timers: Vec<f32>,
    /// Maximum height capacity (terminal rows for this half)
    max_levels: usize,
    /// Smooth visual height that decays gradually (in rows, float for smoothness)
    visual_height: f32,
}

impl BarDecayState {
    fn new(max_levels: usize) -> Self {
        Self {
            level_tiers: vec![-1; max_levels],
            level_timers: vec![0.0; max_levels],
            max_levels,
            visual_height: 0.0,
        }
    }

    /// Resize state arrays if terminal size changed
    fn resize(&mut self, new_max: usize) {
        self.level_tiers.resize(new_max, -1);
        self.level_timers.resize(new_max, 0.0);
        self.max_levels = new_max;
        // Clamp visual height to new bounds
        self.visual_height = self.visual_height.min(new_max as f32);
    }

    /// Update decay timers and transition tiers
    /// Called each frame with delta time
    fn update(&mut self, dt: f32) {
        for level in 0..self.max_levels {
            if self.level_tiers[level] >= 0 {
                self.level_timers[level] -= dt;
                if self.level_timers[level] <= 0.0 {
                    // Transition to next tier
                    self.level_tiers[level] -= 1;
                    if self.level_tiers[level] >= 0 {
                        // Set timer for the new (lower) tier - longer duration
                        self.level_timers[level] = decay_time_for_tier(self.level_tiers[level]);
                    }
                    // If tier dropped to -1, level is now collapsed
                }
            }
        }

        // Smoothly decay visual height using exponential decay
        // Total tier decay time determines how fast height shrinks
        let total_tier_time = (0..NUM_TIERS).map(|t| decay_time_for_tier(t)).sum::<f32>();
        let decay_rate = 1.0 / (total_tier_time * VISUAL_HEIGHT_DECAY_FACTOR);
        self.visual_height *= 1.0 - (decay_rate * dt).min(1.0);
    }

    /// Hit levels 0 through hit_height with fresh tier 3
    /// Levels above hit_height continue their decay
    fn hit(&mut self, hit_height: usize) {
        let hit_level = hit_height.min(self.max_levels);
        let top_tier = (NUM_TIERS - 1) as i8; // Tier 3
        for level in 0..hit_level {
            self.level_tiers[level] = top_tier;
            self.level_timers[level] = decay_time_for_tier(top_tier);
        }
        // Set visual height to new peak
        self.visual_height = self.visual_height.max(hit_height as f32);
    }

    /// Get the tier for a specific level (-1 if collapsed)
    fn tier_at(&self, level: usize) -> i8 {
        if level < self.max_levels {
            self.level_tiers[level]
        } else {
            -1
        }
    }

    /// Get the smooth visual height for rendering
    fn get_visual_height(&self) -> f32 {
        self.visual_height
    }
}

/// Display an error message centered on screen and wait for user to quit
fn display_error_and_wait(
    term: &mut Terminal,
    state: &mut VizState,
    lines: &[&str],
) -> io::Result<()> {
    term.clear();
    let (width, height) = term.size();
    let start_y = height as i32 / 2 - lines.len() as i32 / 2;

    for (line_idx, line) in lines.iter().enumerate() {
        let x_start = width as i32 / 2 - line.len() as i32 / 2;
        for (char_idx, ch) in line.chars().enumerate() {
            term.set(x_start + char_idx as i32, start_y + line_idx as i32, ch, None, false);
        }
    }
    term.present()?;

    loop {
        if let Some((code, mods)) = term.check_key()? {
            if state.handle_key(code, mods) {
                return Ok(());
            }
        }
        term.sleep(ERROR_POLL_INTERVAL);
    }
}

/// Process FFT spectrum data and update bar heights and decay states for one channel.
///
/// Applies logarithmic frequency-to-bar mapping (more bars for bass frequencies),
/// smooths heights with attack/decay, and triggers decay state hits on new peaks.
fn process_channel_spectrum(
    samples: &[f32],
    sample_rate: u32,
    target_heights: &mut [f32],
    prev_hits: &mut [usize],
    decay_states: &mut [BarDecayState],
    available_height: f32,
    half_height_rows: usize,
) {
    let windowed = hann_window(samples);
    let spectrum = match samples_fft_to_spectrum(
        &windowed,
        sample_rate,
        FrequencyLimit::Range(FREQ_MIN_HZ, FREQ_MAX_HZ),
        Some(&divide_by_N_sqrt),
    ) {
        Ok(s) => s,
        Err(_) => return,
    };

    let freq_data: Vec<(f32, f32)> = spectrum.data().iter()
        .map(|(freq, val)| (freq.val(), val.val()))
        .collect();

    if freq_data.is_empty() {
        return;
    }

    let bar_count = target_heights.len();
    for bar_idx in 0..bar_count {
        // Logarithmic mapping: normalized_pos 0..1 maps to log10(1..10) = 0..1
        // This allocates more bars to lower frequencies for musical visualization
        let normalized_pos = bar_idx as f32 / (bar_count - 1).max(1) as f32;
        let log_normalized = (normalized_pos * 9.0 + 1.0).log10();
        let freq_idx = (log_normalized * freq_data.len() as f32) as usize;
        let freq_idx = freq_idx.min(freq_data.len().saturating_sub(1));

        // Average nearby bins (±2) for smoother response
        let bin_start = freq_idx.saturating_sub(2);
        let bin_end = (freq_idx + 3).min(freq_data.len());
        let amplitude_avg: f32 = freq_data[bin_start..bin_end].iter()
            .map(|(_, val)| *val)
            .sum::<f32>() / (bin_end - bin_start).max(1) as f32;

        let raw_target = (amplitude_avg * SENSITIVITY).min(available_height);

        // Smooth the target height for attack/decay feel
        if raw_target > target_heights[bar_idx] {
            target_heights[bar_idx] = target_heights[bar_idx] * (1.0 - ATTACK_RATE) + raw_target * ATTACK_RATE;
        } else {
            target_heights[bar_idx] *= DECAY_RATE;
        }

        // Convert smoothed height to terminal rows
        // Only trigger hit when level INCREASES (not every frame)
        let scaled = (target_heights[bar_idx] / available_height) * half_height_rows as f32;
        let hit_rows = if scaled >= 0.5 {
            (scaled.ceil() as usize).min(half_height_rows)
        } else {
            0
        };

        // Only hit if this is higher than previous - new peak
        if hit_rows > prev_hits[bar_idx] {
            decay_states[bar_idx].hit(hit_rows);
        }
        prev_hits[bar_idx] = hit_rows;
    }
}

/// Get color for a bar cell based on color scheme, position, and decay tier
/// tier: 0-3 where 3 is brightest (newest hit), 0 is dimmest (about to collapse)
fn get_bar_color(
    color_scheme: u8,
    x_ratio: f32,
    tier: i8,
    is_peak: bool,
) -> (Color, bool) {
    if color_scheme == RAINBOW_SCHEME {
        // Rainbow mode: color based on X position, bold for high tiers
        (rainbow_color(x_ratio), tier >= 2)
    } else {
        // Standard schemes: color intensity based on decay tier
        // tier 3 = brightest, tier 0 = dimmest
        let intensity = tier.max(0) as u8;
        scheme_color(color_scheme, intensity.min(3), is_peak)
    }
}

/// Debug logger for audio visualizer diagnostics.
///
/// Writes to a log file when debug mode is enabled. Uses restrictive
/// permissions (0o600) to prevent other users from reading potentially
/// sensitive audio device information.
struct DebugLogger {
    file: Option<File>,
}

impl DebugLogger {
    /// Create a new debug logger, optionally opening a log file.
    fn new(debug_enabled: bool) -> Self {
        use std::fs::OpenOptions;

        let file = if debug_enabled {
            // Try exclusive create first (safe), fall back to truncate (user's own file)
            // Truncate is atomic - no TOCTOU race window unlike delete/recreate
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(DEBUG_LOG_MODE)
                .open(DEBUG_LOG_PATH)
                .or_else(|_| {
                    // File exists - truncate in place (atomic, no race window)
                    OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(DEBUG_LOG_PATH)
                })
                .ok()
        } else {
            None
        };
        Self { file }
    }

    /// Write a formatted message to the log file (if enabled).
    fn log(&mut self, args: std::fmt::Arguments) {
        use std::io::Write;
        if let Some(ref mut f) = self.file {
            let _ = writeln!(f, "{}", args);
            let _ = f.flush();
        }
    }
}

/// Convenience macro for debug logging with format args.
macro_rules! dbg_log {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(format_args!($($arg)*))
    };
}

/// Detect and set a PulseAudio monitor source for system audio capture.
///
/// Finds the monitor source for the default audio output sink and temporarily
/// sets it as the default input source. Returns the original default source
/// (for restoration on exit) and whether a monitor was successfully set.
///
/// # Returns
/// `(original_source, monitor_was_set)` where:
/// - `original_source`: The default source name before modification (if valid)
/// - `monitor_was_set`: Whether a monitor source was found and set as default
fn detect_and_set_monitor_source() -> (Option<String>, bool) {
    // Get original default source for later restoration
    let original_source = std::process::Command::new("pactl")
        .args(["get-default-source"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| is_valid_source_name(s));

    // Get the current default sink (output device) to find its monitor
    let default_sink = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| is_valid_source_name(s));

    // Try to find and set monitor source
    let monitor_set = std::process::Command::new("pactl")
        .args(["list", "sources", "short"])
        .output()
        .ok()
        .and_then(|output| {
            if !output.status.success() {
                return None;
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            // First, try to find the monitor for the current default sink
            if let Some(ref sink) = default_sink {
                let expected_monitor = format!("{}.monitor", sink);
                for line in &lines {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 2 && parts[1] == expected_monitor && is_valid_source_name(parts[1]) {
                        let _ = std::process::Command::new("pactl")
                            .args(["set-default-source", parts[1]])
                            .output();
                        return Some(parts[1].to_string());
                    }
                }
            }

            // Fallback: find any monitor source
            for line in &lines {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 && parts[1].contains(".monitor") && is_valid_source_name(parts[1]) {
                    let _ = std::process::Command::new("pactl")
                        .args(["set-default-source", parts[1]])
                        .output();
                    return Some(parts[1].to_string());
                }
            }

            None
        });

    (original_source, monitor_set.is_some())
}

/// Restore the original PulseAudio default source.
fn restore_original_source(original_source: &Option<String>) {
    if let Some(ref orig) = original_source {
        let _ = std::process::Command::new("pactl")
            .args(["set-default-source", orig])
            .output();
    }
}

/// RAII guard to restore PulseAudio monitor source on drop.
/// Ensures cleanup happens on all exit paths (normal exit, early return, panic).
struct MonitorSourceGuard {
    original_source: Option<String>,
    should_restore: bool,
}

impl MonitorSourceGuard {
    fn new(original_source: Option<String>, should_restore: bool) -> Self {
        Self {
            original_source,
            should_restore,
        }
    }
}

impl Drop for MonitorSourceGuard {
    fn drop(&mut self) {
        if self.should_restore {
            restore_original_source(&self.original_source);
        }
    }
}

/// Resize all tracking arrays to a new bar count
fn resize_tracking_arrays(
    target_heights_left: &mut Vec<f32>,
    target_heights_right: &mut Vec<f32>,
    prev_hit_left: &mut Vec<usize>,
    prev_hit_right: &mut Vec<usize>,
    new_size: usize,
) {
    target_heights_left.resize(new_size, 0.0);
    target_heights_right.resize(new_size, 0.0);
    prev_hit_left.resize(new_size, 0);
    prev_hit_right.resize(new_size, 0);
}

/// Direction for bar rendering (up from center or down from center)
#[derive(Clone, Copy)]
enum BarDirection {
    Up,   // Left channel: grows upward from center
    Down, // Right channel: grows downward from center
}

/// Render a single bar with sub-cell resolution using partial block characters.
///
/// Draws both full blocks and a partial block at the tip for smooth animation.
/// Color is determined by the decay tier at each level.
fn render_channel_bar(
    term: &mut Terminal,
    decay_state: &BarDecayState,
    bar_x: i32,
    bar_width: usize,
    x_ratio: f32,
    half_height: f32,
    center_y: i32,
    max_height: usize,
    terminal_height: u16,
    direction: BarDirection,
    bar_chars: &[char],
    color_scheme: u8,
) {
    let visual_height = decay_state.get_visual_height();
    let full_blocks = visual_height as usize;
    let frac = visual_height.fract();

    // Full blocks
    for y in 0..full_blocks.min(max_height) {
        let tier = decay_state.tier_at(y);
        let render_tier = if tier < 0 { 0 } else { tier };

        let y_ratio = y as f32 / half_height;
        let is_peak = y_ratio > PEAK_THRESHOLD;
        let (color, bold) = get_bar_color(color_scheme, x_ratio, render_tier, is_peak);

        let screen_y = match direction {
            BarDirection::Up => center_y - 1 - y as i32,
            BarDirection::Down => center_y + y as i32,
        };

        let in_bounds = match direction {
            BarDirection::Up => screen_y >= 0,
            BarDirection::Down => screen_y < terminal_height as i32,
        };

        if in_bounds {
            for x_offset in 0..bar_width as i32 {
                term.set(bar_x + x_offset, screen_y, '█', Some(color), bold);
            }
        }
    }

    // Partial block at tip
    if frac > 0.0 && full_blocks < max_height {
        let tier = decay_state.tier_at(full_blocks);
        let render_tier = if tier < 0 { 0 } else { tier };
        let char_idx = (frac * (bar_chars.len() - 1) as f32).round() as usize;
        let char_idx = char_idx.min(bar_chars.len() - 1);
        let (color, bold) = get_bar_color(color_scheme, x_ratio, render_tier, false);

        let screen_y = match direction {
            BarDirection::Up => center_y - 1 - full_blocks as i32,
            BarDirection::Down => center_y + full_blocks as i32,
        };

        let in_bounds = match direction {
            BarDirection::Up => screen_y >= 0,
            BarDirection::Down => screen_y < terminal_height as i32,
        };

        if in_bounds {
            for x_offset in 0..bar_width as i32 {
                term.set(bar_x + x_offset, screen_y, bar_chars[char_idx], Some(color), bold);
            }
        }
    }
}

/// Help text for audio visualizer
const HELP: &str = "\
AUDIO SPECTRUM
─────────────────
←/→  Bar count -/+";

/// Run the audio spectrum visualizer
///
/// Captures system audio via PulseAudio/PipeWire monitor source and displays
/// a real-time frequency spectrum with CAVA-style bar visualization.
///
/// # Audio Source Detection
/// Automatically detects and temporarily sets a monitor source for capturing
/// system audio output. The original default source is restored on exit.
///
/// # Arguments
/// * `term` - Terminal instance for rendering
/// * `config` - Visualization configuration (speed, debug mode, etc.)
///
/// # Errors
/// Returns an error if:
/// - No audio input device is found
/// - Audio stream fails to build or start
/// - Audio device reports invalid channel count
pub fn run(term: &mut Terminal, config: &FractalConfig) -> io::Result<()> {
    let mut log = DebugLogger::new(config.debug);

    dbg_log!(log, "Starting audio visualizer");

    let mut state = VizState::new(config.time_step, HELP);


    // Suppress ALSA debug spam by redirecting stderr temporarily during device enumeration
    // This prevents ALSA lib errors from corrupting the terminal display
    dbg_log!(log, "Suppressing ALSA errors");
    let _stderr_guard = suppress_alsa_errors();

    // Set up audio capture with stereo support
    dbg_log!(log, "Creating audio buffer");
    let audio_buffer = Arc::new(Mutex::new(StereoAudioBuffer::new()));
    let audio_buffer_clone = Arc::clone(&audio_buffer);

    // Try to set up audio input
    dbg_log!(log, "Getting default host");
    let host = cpal::default_host();
    dbg_log!(log, "Host: {:?}", host.id());

    // Like CAVA, try to auto-select a monitor source for system audio
    dbg_log!(log, "Looking for monitor source via pactl");
    let (original_source, monitor_was_set) = detect_and_set_monitor_source();
    dbg_log!(log, "Monitor source detection: original={:?}, set={}", original_source, monitor_was_set);

    // Create RAII guard immediately - will restore on any exit path (normal, early return, panic)
    let _source_guard = MonitorSourceGuard::new(original_source, monitor_was_set);

    // Now get the default device (which should be the monitor if we found one)
    dbg_log!(log, "Getting default input device");
    let device = host.default_input_device();

    let device = match device {
        Some(d) => d,
        None => {
            return display_error_and_wait(term, &mut state, &[
                "No audio input device found",
                "",
                "For system audio, set a monitor source as default:",
                "  pactl set-default-source \\",
                "    $(pactl list sources short | grep monitor | head -1 | cut -f1)",
                "",
                "Or use pavucontrol to select the monitor source",
            ]);
        }
    };

    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    dbg_log!(log, "Device: {}", device_name);

    // Get device's default config, or construct a reasonable default
    dbg_log!(log, "Getting device config");
    let supported_config = device.default_input_config().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("No supported config: {}", e))
    })?;

    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();
    dbg_log!(log, "Config: {}Hz, {} channels", sample_rate, channels);

    // Validate channel count to prevent panic in audio callback
    if channels == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Audio device reported 0 channels",
        ));
    }

    let stream_config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Build input stream - keep stereo channels separate
    dbg_log!(log, "Building input stream");
    let stream = device.build_input_stream(
        &stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Ok(mut buffer) = audio_buffer_clone.lock() {
                if channels == 1 {
                    buffer.push_mono_samples(data);
                } else if channels == 2 {
                    // Stereo: keep left and right separate
                    buffer.push_stereo_samples(data);
                } else {
                    // Multi-channel: take first two channels as L/R
                    // Guard against empty chunks to prevent panic
                    let stereo: Vec<f32> = data.chunks(channels as usize)
                        .flat_map(|chunk| {
                            if chunk.is_empty() {
                                [0.0, 0.0]
                            } else {
                                [chunk[0], chunk.get(1).copied().unwrap_or(chunk[0])]
                            }
                        })
                        .collect();
                    buffer.push_stereo_samples(&stereo);
                }
            }
        },
        |err| eprintln!("Audio stream error: {}", err),
        None,
    );

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let error_msg = format!("Audio stream error: {}", e);
            return display_error_and_wait(term, &mut state, &[&error_msg]);
        }
    };

    dbg_log!(log, "Starting stream playback");
    if let Err(e) = stream.play() {
        drop(_stderr_guard); // Restore stderr for error display
        let error_msg = format!("Failed to start audio: {}", e);
        return display_error_and_wait(term, &mut state, &[&error_msg]);
    }

    // Audio init complete - restore stderr and clear any garbage from screen
    drop(_stderr_guard);
    term.clear_screen()?;

    let (init_w, init_h) = term.size();
    let mut prev_w = init_w;
    let mut prev_h = init_h;

    // Maximum height levels per channel (half the screen height)
    let max_levels = (init_h / 2).max(1) as usize;

    // Per-bar decay state for left and right channels
    // Each bar tracks per-level tier and decay timer independently
    let mut decay_left: Vec<BarDecayState> = (0..MAX_BARS)
        .map(|_| BarDecayState::new(max_levels))
        .collect();
    let mut decay_right: Vec<BarDecayState> = (0..MAX_BARS)
        .map(|_| BarDecayState::new(max_levels))
        .collect();

    // Smoothed target heights for hit detection (audio level tracking)
    let mut target_heights_left: Vec<f32> = vec![0.0; MAX_BARS];
    let mut target_heights_right: Vec<f32> = vec![0.0; MAX_BARS];

    // Previous hit heights - only trigger new hits when level INCREASES
    let mut prev_hit_left: Vec<usize> = vec![0; MAX_BARS];
    let mut prev_hit_right: Vec<usize> = vec![0; MAX_BARS];

    // CAVA-style bar configuration - adjustable with left/right arrows
    let mut num_bars: usize = DEFAULT_BAR_COUNT;

    // Frame timing for decay updates
    let mut last_frame = std::time::Instant::now();

    // Preallocated sample buffers to avoid per-frame allocations
    let mut samples_left = vec![0.0f32; BUFFER_SIZE];
    let mut samples_right = vec![0.0f32; BUFFER_SIZE];

    // Bar rendering characters - matched sets for symmetric animation
    // Lower blocks fill from bottom up (for top half): 1/8, 1/2, full
    let bar_chars_lower = ['▁', '▄', '█'];
    // Upper blocks fill from top down (for bottom half): 1/8, 1/2, full
    let bar_chars_upper = ['▔', '▀', '█'];

    loop {
        let (width, height) = crossterm::terminal::size().unwrap_or(term.size());

        // Calculate frame delta time for decay updates
        let now = std::time::Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32();
        last_frame = now;

        if width != prev_w || height != prev_h {
            term.resize(width, height);
            term.clear_screen()?;
            prev_w = width;
            prev_h = height;

            // Resize decay states for new terminal height
            let new_max_levels = (height / 2).max(1) as usize;
            for state in &mut decay_left {
                state.resize(new_max_levels);
            }
            for state in &mut decay_right {
                state.resize(new_max_levels);
            }
        }

        // Ensure arrays match current bar count
        if target_heights_left.len() != num_bars {
            resize_tracking_arrays(
                &mut target_heights_left,
                &mut target_heights_right,
                &mut prev_hit_left,
                &mut prev_hit_right,
                num_bars,
            );
        }

        if let Some((code, mods)) = term.check_key()? {
            match code {
                // Left/Right arrows adjust bar count (resize handled by frame loop above)
                crossterm::event::KeyCode::Left => {
                    num_bars = (num_bars.saturating_sub(BAR_ADJUST_STEP)).max(MIN_BARS);
                }
                crossterm::event::KeyCode::Right => {
                    num_bars = (num_bars + BAR_ADJUST_STEP).min(MAX_BARS).min(width as usize);
                }
                _ => {
                    if state.handle_key(code, mods) {
                        break;
                    }
                }
            }
        }

        if state.paused {
            term.sleep(ERROR_POLL_INTERVAL);
            continue;
        }

        term.clear();

        // Get current audio samples for both channels
        if let Ok(buffer) = audio_buffer.lock() {
            buffer.copy_samples(&mut samples_left, &mut samples_right);
        } else {
            samples_left.fill(0.0);
            samples_right.fill(0.0);
        }

        let current_bar_count = target_heights_left.len();
        let available_height = height.saturating_sub(1) as f32;
        let half_height_rows = (height / 2).max(1) as usize;

        // Process left channel FFT and update decay states
        process_channel_spectrum(
            &samples_left,
            sample_rate,
            &mut target_heights_left,
            &mut prev_hit_left,
            &mut decay_left,
            available_height,
            half_height_rows,
        );

        // Process right channel FFT and update decay states
        process_channel_spectrum(
            &samples_right,
            sample_rate,
            &mut target_heights_right,
            &mut prev_hit_right,
            &mut decay_right,
            available_height,
            half_height_rows,
        );

        // Update all decay timers
        for bar_idx in 0..current_bar_count.min(decay_left.len()) {
            decay_left[bar_idx].update(dt);
            decay_right[bar_idx].update(dt);
        }

        // Calculate bar width and spacing based on current bar count
        // Each bar needs BAR_GAP space after it (except the last one)
        // Minimum bar width is 1, so max bars that fit = width / (1 + BAR_GAP)
        let total_width = width as usize;
        let max_bars_that_fit = total_width / (1 + BAR_GAP);
        let effective_bar_count = current_bar_count.min(max_bars_that_fit).max(1);
        let total_gaps = if effective_bar_count > 1 { (effective_bar_count - 1) * BAR_GAP } else { 0 };
        let available_for_bars = total_width.saturating_sub(total_gaps);
        let bar_width = (available_for_bars / effective_bar_count).max(1);

        // Center line separating left (top) and right (bottom) channels
        let center_y = height as i32 / 2;
        // Half height available for each channel
        let half_height = (height as i32 / 2).max(1) as f32;

        // Render stereo bars: LEFT channel on TOP, RIGHT channel on BOTTOM
        let max_top = center_y as usize;
        let max_bottom = (height as i32 - center_y) as usize;

        for bar_idx in 0..effective_bar_count {
            let bar_x = (bar_idx * (bar_width + BAR_GAP)) as i32;
            let x_ratio = bar_idx as f32 / effective_bar_count.max(1) as f32;

            // Left channel (top half, grows upward)
            render_channel_bar(
                term, &decay_left[bar_idx], bar_x, bar_width, x_ratio,
                half_height, center_y, max_top, height,
                BarDirection::Up, &bar_chars_lower, state.color_scheme(),
            );

            // Right channel (bottom half, grows downward)
            render_channel_bar(
                term, &decay_right[bar_idx], bar_x, bar_width, x_ratio,
                half_height, center_y, max_bottom, height,
                BarDirection::Down, &bar_chars_upper, state.color_scheme(),
            );
        }

        state.render_help(term, width, height);
        term.present()?;
        term.sleep(state.speed.max(MIN_FRAME_TIME as f32));
    }

    // Note: _source_guard handles restoration automatically via Drop
    Ok(())
}
