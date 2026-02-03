use crate::colors::ColorState;
use crate::help::render_help_overlay;
use crate::terminal::Terminal;
use crate::monitor::{build_help, MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Rect, draw_meter_btop_scheme, format_bytes,
    cpu_gradient_color_scheme, text_color_scheme, muted_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::fs;
use std::io;

pub struct MemInfo {
    pub mem_total: u64,
    pub mem_available: u64,
    pub mem_free: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl MemInfo {
    pub fn mem_used(&self) -> u64 {
        self.mem_total.saturating_sub(self.mem_available)
    }

    pub fn mem_percent(&self) -> f32 {
        if self.mem_total > 0 {
            (self.mem_used() as f32 / self.mem_total as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn cached_percent(&self) -> f32 {
        if self.mem_total > 0 {
            ((self.cached + self.buffers) as f32 / self.mem_total as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }

    pub fn swap_percent(&self) -> f32 {
        if self.swap_total > 0 {
            (self.swap_used() as f32 / self.swap_total as f32) * 100.0
        } else {
            0.0
        }
    }
}

pub struct MemMonitor {
    pub info: MemInfo,
}

impl MemMonitor {
    pub fn new() -> Self {
        Self {
            info: MemInfo {
                mem_total: 0, mem_available: 0, mem_free: 0,
                buffers: 0, cached: 0, swap_total: 0, swap_free: 0,
            },
        }
    }

    fn parse_kb(s: &str) -> u64 {
        s.split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0) * 1024
    }

    pub fn update(&mut self) -> io::Result<()> {
        let content = fs::read_to_string("/proc/meminfo")?;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                self.info.mem_total = Self::parse_kb(line);
            } else if line.starts_with("MemAvailable:") {
                self.info.mem_available = Self::parse_kb(line);
            } else if line.starts_with("MemFree:") {
                self.info.mem_free = Self::parse_kb(line);
            } else if line.starts_with("Buffers:") {
                self.info.buffers = Self::parse_kb(line);
            } else if line.starts_with("Cached:") && !line.starts_with("SwapCached:") {
                self.info.cached = Self::parse_kb(line);
            } else if line.starts_with("SwapTotal:") {
                self.info.swap_total = Self::parse_kb(line);
            } else if line.starts_with("SwapFree:") {
                self.info.swap_free = Self::parse_kb(line);
            }
        }

        Ok(())
    }

    pub fn render_fullscreen(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        self.render_at(term, 0, 0, w, h, colors);
    }

    #[allow(dead_code)]
    pub fn render(&self, term: &mut Terminal, bx: &Rect, colors: &ColorState) {
        let x = bx.inner_x();
        let y = bx.inner_y();
        let w = bx.inner_width() as usize;
        let h = bx.inner_height() as usize;
        self.render_at(term, x, y, w, h, colors);
    }

    fn render_at(&self, term: &mut Terminal, x: i32, y: i32, w: usize, h: usize, colors: &ColorState) {
        if h < 4 || w < 30 { return; }

        // Use full width
        let panel_w = w;
        let panel_x = x;

        // Calculate info panel height
        // Title(1) + Used(1) + Cached(1) + Buffers(1) + Free(1) + blank(1) + Swap title(1) + Swap(1) = 8
        let has_swap = self.info.swap_total > 0;
        let info_height = if has_swap { 8 } else { 5 };

        // Position info panel vertically centered
        let info_y = y + ((h as i32 - info_height) / 2).max(0);

        let mut cy = info_y;

        // Memory title with total right-aligned
        term.set_str(panel_x, cy, "Memory", Some(text_color_scheme(colors)), true);
        let total_str = format_bytes(self.info.mem_total);
        term.set_str(panel_x + panel_w as i32 - total_str.len() as i32, cy, &total_str, Some(muted_color_scheme(colors)), false);
        cy += 1;

        // Used memory with meter
        let used_pct = self.info.mem_percent();
        self.draw_mem_row(term, panel_x, cy, panel_w, "Used", self.info.mem_used(), used_pct, colors, true);
        cy += 1;

        // Cached
        let cached_pct = self.info.cached_percent();
        self.draw_mem_row(term, panel_x, cy, panel_w, "Cached", self.info.cached, cached_pct, colors, false);
        cy += 1;

        // Buffers
        let buffers_pct = if self.info.mem_total > 0 {
            (self.info.buffers as f32 / self.info.mem_total as f32) * 100.0
        } else { 0.0 };
        self.draw_mem_row(term, panel_x, cy, panel_w, "Buffers", self.info.buffers, buffers_pct, colors, false);
        cy += 1;

        // Free
        let free_pct = if self.info.mem_total > 0 {
            (self.info.mem_free as f32 / self.info.mem_total as f32) * 100.0
        } else { 0.0 };
        self.draw_mem_row(term, panel_x, cy, panel_w, "Free", self.info.mem_free, free_pct, colors, false);
        cy += 1;

        // Swap section (if present)
        if has_swap {
            cy += 1; // Blank line

            term.set_str(panel_x, cy, "Swap", Some(text_color_scheme(colors)), true);
            let swap_total_str = format_bytes(self.info.swap_total);
            term.set_str(panel_x + panel_w as i32 - swap_total_str.len() as i32, cy, &swap_total_str, Some(muted_color_scheme(colors)), false);
            cy += 1;

            let swap_pct = self.info.swap_percent();
            self.draw_mem_row(term, panel_x, cy, panel_w, "Used", self.info.swap_used(), swap_pct, colors, true);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_mem_row(&self, term: &mut Terminal, x: i32, y: i32, width: usize, label: &str, bytes: u64, percent: f32, colors: &ColorState, use_gradient: bool) {
        // Layout: Label(8) + Meter(dynamic) + Pct(6) + Size(9)
        // Meter fills space between label and pct+size
        let label_w = 8;
        let pct_w = 6;  // " 17% " with space
        let size_w = 9; // "448.8MiB" + padding
        let meter_w = width.saturating_sub(label_w + pct_w + size_w);

        let mut pos = x;

        // Label (8 chars)
        let label_str = format!("{:<8}", label);
        term.set_str(pos, y, &label_str, Some(muted_color_scheme(colors)), false);
        pos += label_w as i32;

        // Get color based on scheme
        let color = if use_gradient {
            cpu_gradient_color_scheme(percent, colors)
        } else if colors.is_mono() {
            Color::AnsiValue(12)  // Blue for non-gradient items in mono
        } else {
            cpu_gradient_color_scheme(50.0, colors)  // Mid-intensity for non-gradient
        };

        // Meter (dynamic width)
        if meter_w > 0 {
            draw_meter_btop_scheme(term, pos, y, meter_w, percent, colors);
            pos += meter_w as i32;
        }

        // Percentage (6 chars with trailing space)
        let pct_str = format!("{:4.0}% ", percent);
        term.set_str(pos, y, &pct_str, Some(color), false);
        pos += pct_w as i32;

        // Size right-aligned
        let size_str = format_bytes(bytes);
        let size_pad = size_w.saturating_sub(size_str.len());
        term.set_str(pos + size_pad as i32, y, &size_str, Some(muted_color_scheme(colors)), false);
    }
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(0.5));
    let mut monitor = MemMonitor::new();
    let help_text = build_help("MEMORY MONITOR", "");
    let mut show_help = false;

    monitor.update()?;
    std::thread::sleep(std::time::Duration::from_millis(100));

    loop {
        if let Ok(Some((code, mods))) = term.check_key() {
            if code == crossterm::event::KeyCode::Char('?') {
                show_help = !show_help;
            } else if state.handle_key(code, mods) {
                break;
            }
        }

        if let Ok((new_w, new_h)) = size() {
            let (cur_w, cur_h) = term.size();
            if new_w != cur_w || new_h != cur_h {
                term.resize(new_w, new_h);
                term.clear_screen()?;
            }
        }

        if !state.paused {
            monitor.update()?;
        }

        term.clear();

        // Render without border
        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);

        if show_help {
            let (w, h) = term.size();
            render_help_overlay(&mut term, w, h, &help_text);
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
