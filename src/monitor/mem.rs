use crate::colors::ColorState;
use crate::help::HelpSpec;
use crate::monitor::layout::{
    cpu_gradient_color_scheme, draw_meter_btop_scheme, format_bytes, muted_color_scheme,
    text_color_scheme, Rect,
};
use crate::monitor::{MonitorAction, MonitorConfig, MonitorState};
use crate::terminal::Terminal;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::fs;
use std::io;

// Removing this panel's Available row must not make the shared layout entry
// points dead in non-test builds; their definitions remain for other panels.
const _: fn(&mut Terminal, i32, i32, usize, f32, &ColorState) =
    crate::monitor::layout::draw_meter_headroom_scheme;
const _: fn(usize) -> (usize, usize) = crate::monitor::layout::split_row_for_graph;

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
            (self.cached as f32 / self.mem_total as f32) * 100.0
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

/// How a memory row reads its own value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MeterStyle {
    /// Consumption: more is worse, colored by the usage gradient.
    Usage,
    /// Informational (cached, buffers): neither good nor bad.
    Neutral,
}

pub struct MemMonitor {
    pub info: MemInfo,
}

impl MemMonitor {
    pub fn new() -> Self {
        Self {
            info: MemInfo {
                mem_total: 0,
                mem_available: 0,
                mem_free: 0,
                buffers: 0,
                cached: 0,
                swap_total: 0,
                swap_free: 0,
            },
        }
    }

    fn parse_kb(s: &str) -> u64 {
        s.split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
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

    fn render_at(
        &self,
        term: &mut Terminal,
        x: i32,
        y: i32,
        w: usize,
        h: usize,
        colors: &ColorState,
    ) {
        if h < 4 {
            return;
        }

        // Use full width
        let panel_w = w;
        let panel_x = x;

        // Calculate info panel height
        // Title(1) + Used(1) + Cached(1) + Buffers(1) + Swap(1) = 5
        let has_swap = self.info.swap_total > 0 && h >= 5;
        let info_height = if has_swap { 5 } else { 4 };

        let used_size = format_bytes(self.info.mem_used());
        let cached_size = format_bytes(self.info.cached);
        let buffers_size = format_bytes(self.info.buffers);
        let swap_size =
            has_swap.then(|| format_used_total_bytes(self.info.swap_used(), self.info.swap_total));
        let size_w = [
            9,
            used_size.len(),
            cached_size.len(),
            buffers_size.len(),
            swap_size.as_ref().map_or(0, String::len),
        ]
        .into_iter()
        .max()
        .unwrap_or(9);

        // Preserve the original five-column meter floor while ensuring every
        // accepted width can show the widest value without clipping it.
        if w < 10 + 5 + 6 + size_w {
            return;
        }

        // Position info panel vertically centered
        let info_y = y + ((h as i32 - info_height) / 2).max(0);

        let mut cy = info_y;

        // Memory title with total right-aligned
        term.set_str(panel_x, cy, "Memory", Some(text_color_scheme(colors)), true);
        let total_str = format_bytes(self.info.mem_total);
        term.set_str(
            panel_x + panel_w as i32 - total_str.len() as i32,
            cy,
            &total_str,
            Some(muted_color_scheme(colors)),
            false,
        );
        cy += 1;

        // Used memory with meter
        let used_pct = self.info.mem_percent();
        self.draw_mem_row(
            term,
            panel_x,
            cy,
            panel_w,
            "Used",
            &used_size,
            size_w,
            used_pct,
            colors,
            MeterStyle::Usage,
        );
        cy += 1;

        // Cached
        let cached_pct = self.info.cached_percent();
        self.draw_mem_row(
            term,
            panel_x,
            cy,
            panel_w,
            "Cached",
            &cached_size,
            size_w,
            cached_pct,
            colors,
            MeterStyle::Neutral,
        );
        cy += 1;

        // Buffers
        let buffers_pct = if self.info.mem_total > 0 {
            (self.info.buffers as f32 / self.info.mem_total as f32) * 100.0
        } else {
            0.0
        };
        self.draw_mem_row(
            term,
            panel_x,
            cy,
            panel_w,
            "Buffers",
            &buffers_size,
            size_w,
            buffers_pct,
            colors,
            MeterStyle::Neutral,
        );
        cy += 1;

        // Swap (if present)
        if let Some(swap_size) = swap_size {
            let swap_pct = self.info.swap_percent();
            self.draw_mem_row(
                term,
                panel_x,
                cy,
                panel_w,
                "Swap",
                &swap_size,
                size_w,
                swap_pct,
                colors,
                MeterStyle::Usage,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_mem_row(
        &self,
        term: &mut Terminal,
        x: i32,
        y: i32,
        width: usize,
        label: &str,
        size_str: &str,
        size_w: usize,
        percent: f32,
        colors: &ColorState,
        style: MeterStyle,
    ) {
        // Layout: Label(10) + Meter(dynamic) + Pct(6) + Size(dynamic, at least 9)
        // Meter fills space between label and pct+size
        let label_w = 10;
        let pct_w = 6; // " 17% " with space
        let elastic = width.saturating_sub(label_w + pct_w + size_w);

        let mut pos = x;

        // Label
        let label_str = format!("{:<10}", label);
        term.set_str(pos, y, &label_str, Some(muted_color_scheme(colors)), false);
        pos += label_w as i32;

        // Get color based on scheme
        let color = match style {
            MeterStyle::Usage => cpu_gradient_color_scheme(percent, colors),
            MeterStyle::Neutral if colors.is_mono() => {
                Color::AnsiValue(12) // Blue for non-gradient items in mono
            }
            MeterStyle::Neutral => cpu_gradient_color_scheme(50.0, colors), // Mid-intensity
        };

        // Meter
        draw_meter_btop_scheme(term, pos, y, elastic, percent, colors);
        pos += elastic as i32;

        // Percentage (6 chars with trailing space)
        let pct_str = format!("{:4.0}% ", percent);
        term.set_str(pos, y, &pct_str, Some(color), false);
        pos += pct_w as i32;

        // Size right-aligned
        let size_pad = size_w.saturating_sub(size_str.len());
        term.set_str(
            pos + size_pad as i32,
            y,
            size_str,
            Some(muted_color_scheme(colors)),
            false,
        );
    }
}

/// Format a used/total pair with the total's adaptive `format_bytes` unit.
fn format_used_total_bytes(used: u64, total: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    let total_str = format_bytes(total);
    let used_str = if total >= TIB {
        format!("{:.1}", used as f64 / TIB as f64)
    } else if total >= GIB {
        format!("{:.1}", used as f64 / GIB as f64)
    } else if total >= MIB {
        format!("{:.1}", used as f64 / MIB as f64)
    } else if total >= KIB {
        format!("{:.1}", used as f64 / KIB as f64)
    } else {
        used.to_string()
    };

    format!("{used_str}/{total_str}")
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step, 0.5);
    let mut monitor = MemMonitor::new();
    const HELP: HelpSpec = HelpSpec::monitor("MEMORY MONITOR", &[]);

    loop {
        let mut action = MonitorAction::None;
        if let Ok(Some((code, mods))) = term.check_key() {
            action = state.handle_key(code, mods);
            if action == MonitorAction::Quit {
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

        if state.should_sample(action) {
            state.record_sample(monitor.update());
        }

        term.clear();

        // Render without border
        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);
        state.render_help(&mut term, w, h, &HELP);

        term.present()?;
        term.sleep(state.poll_delay());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{format_used_total_bytes, MemInfo};

    #[test]
    fn cache_percentage_matches_displayed_cache_bytes() {
        let info = MemInfo {
            mem_total: 1000,
            mem_available: 400,
            mem_free: 100,
            buffers: 100,
            cached: 300,
            swap_total: 0,
            swap_free: 0,
        };

        assert!((info.cached_percent() - 30.0).abs() < 0.001);
        assert!((info.mem_percent() - 60.0).abs() < 0.001);
    }

    #[test]
    fn swap_usage_uses_the_totals_adaptive_unit_once() {
        const MIB: u64 = 1024 * 1024;
        const GIB: u64 = MIB * 1024;

        assert_eq!(
            format_used_total_bytes(22 * GIB / 10, 20 * GIB),
            "2.2/20.0GiB"
        );
        assert_eq!(format_used_total_bytes(512 * MIB, 20 * GIB), "0.5/20.0GiB");
    }
}
