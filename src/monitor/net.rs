use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::{MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Box, draw_meter_btop_scheme, format_rate, format_bytes,
    cpu_gradient_color_scheme, text_color_scheme, muted_color_scheme, header_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::collections::HashMap;
use std::fs;
use std::io;

struct NetStats {
    rx_bytes: u64,
    tx_bytes: u64,
}

pub struct NetMonitor {
    prev_stats: HashMap<String, NetStats>,
    interfaces: Vec<String>,
    pub rx_rates: HashMap<String, f64>,
    pub tx_rates: HashMap<String, f64>,
    pub total_rx: HashMap<String, u64>,
    pub total_tx: HashMap<String, u64>,
    pub total_rx_rate: f64,
    pub total_tx_rate: f64,
    pub peak_rx_rate: f64,
    pub peak_tx_rate: f64,
}

impl NetMonitor {
    pub fn new() -> Self {
        Self {
            prev_stats: HashMap::new(),
            interfaces: Vec::new(),
            rx_rates: HashMap::new(),
            tx_rates: HashMap::new(),
            total_rx: HashMap::new(),
            total_tx: HashMap::new(),
            total_rx_rate: 0.0,
            total_tx_rate: 0.0,
            peak_rx_rate: 1024.0 * 1024.0, // Start with 1MB/s as minimum scale
            peak_tx_rate: 1024.0 * 1024.0,
        }
    }

    pub fn update(&mut self, interval: f32) -> io::Result<()> {
        let content = fs::read_to_string("/proc/net/dev")?;
        let mut current_stats: HashMap<String, NetStats> = HashMap::new();
        let mut interfaces: Vec<String> = Vec::new();

        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }

            let name = parts[0].trim_end_matches(':');
            if name == "lo" {
                continue;
            }

            let rx_bytes: u64 = parts[1].parse().unwrap_or(0);
            let tx_bytes: u64 = parts[9].parse().unwrap_or(0);

            // Skip interfaces with no traffic
            if rx_bytes == 0 && tx_bytes == 0 {
                continue;
            }

            interfaces.push(name.to_string());
            current_stats.insert(name.to_string(), NetStats { rx_bytes, tx_bytes });
            self.total_rx.insert(name.to_string(), rx_bytes);
            self.total_tx.insert(name.to_string(), tx_bytes);
        }

        self.interfaces = interfaces;
        self.total_rx_rate = 0.0;
        self.total_tx_rate = 0.0;

        for (name, stats) in &current_stats {
            if let Some(prev) = self.prev_stats.get(name) {
                let rx_diff = stats.rx_bytes.saturating_sub(prev.rx_bytes);
                let tx_diff = stats.tx_bytes.saturating_sub(prev.tx_bytes);
                let rx_rate = rx_diff as f64 / interval as f64;
                let tx_rate = tx_diff as f64 / interval as f64;

                self.rx_rates.insert(name.clone(), rx_rate);
                self.tx_rates.insert(name.clone(), tx_rate);

                self.total_rx_rate += rx_rate;
                self.total_tx_rate += tx_rate;
            }
        }

        self.prev_stats = current_stats;

        // Update peak rates for auto-scaling (with decay)
        if self.total_rx_rate > self.peak_rx_rate {
            self.peak_rx_rate = self.total_rx_rate;
        } else {
            self.peak_rx_rate = (self.peak_rx_rate * 0.999).max(1024.0 * 1024.0);
        }
        if self.total_tx_rate > self.peak_tx_rate {
            self.peak_tx_rate = self.total_tx_rate;
        } else {
            self.peak_tx_rate = (self.peak_tx_rate * 0.999).max(1024.0 * 1024.0);
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn render(&self, term: &mut Terminal, bx: &Box, colors: &ColorState) {
        let x = bx.inner_x();
        let y = bx.inner_y();
        let w = bx.inner_width() as usize;
        let h = bx.inner_height() as usize;
        self.render_at(term, x, y, w, h, colors);
    }

    pub fn render_fullscreen(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        self.render_at(term, 0, 0, w, h, colors);
    }

    fn render_at(&self, term: &mut Terminal, x: i32, y: i32, w: usize, h: usize, colors: &ColorState) {
        if h < 4 || w < 30 { return; }

        // Calculate panel height: Title(1) + Download(1) + Upload(1) + blank + per-interface lines
        let num_ifaces = self.interfaces.len().min(4); // Show max 4 interfaces
        // Each interface: name(1) + download(1) + upload(1) = 3 lines
        let panel_height = 3 + if num_ifaces > 1 { 1 + num_ifaces * 3 } else { 0 };

        // Content width: cap at 80 chars for readability
        let content_w = w.min(80);

        // Horizontally center
        let start_x = x + ((w as i32 - content_w as i32) / 2).max(0);

        // Vertically center (allow negative to clip top and bottom equally)
        let start_y = y + (h as i32 - panel_height as i32) / 2;
        let mut cy = start_y;

        // Title with total transferred
        let total_rx: u64 = self.total_rx.values().sum();
        let total_tx: u64 = self.total_tx.values().sum();
        term.set_str(start_x, cy, "Network", Some(text_color_scheme(colors)), true);
        let totals_str = format!("↓{} ↑{}", format_bytes(total_rx), format_bytes(total_tx));
        term.set_str(start_x + content_w as i32 - totals_str.len() as i32, cy, &totals_str, Some(muted_color_scheme(colors)), false);
        cy += 1;

        // Download rate
        let rx_pct = ((self.total_rx_rate / self.peak_rx_rate) * 100.0).min(100.0) as f32;
        self.draw_net_row(term, start_x, cy, content_w, "Download", rx_pct, self.total_rx_rate, colors, true);
        cy += 1;

        // Upload rate
        let tx_pct = ((self.total_tx_rate / self.peak_tx_rate) * 100.0).min(100.0) as f32;
        self.draw_net_row(term, start_x, cy, content_w, "Upload", tx_pct, self.total_tx_rate, colors, false);
        cy += 1;

        // Per-interface breakdown (if multiple interfaces)
        if num_ifaces > 1 {
            cy += 1; // Blank line

            for iface in self.interfaces.iter().take(4) {
                let iface_rx = self.rx_rates.get(iface).copied().unwrap_or(0.0);
                let iface_tx = self.tx_rates.get(iface).copied().unwrap_or(0.0);

                // Interface name as label
                term.set_str(start_x, cy, iface, Some(header_color_scheme(colors)), false);
                cy += 1;

                // Download for this interface
                let iface_rx_pct = ((iface_rx / self.peak_rx_rate) * 100.0).min(100.0) as f32;
                self.draw_net_row(term, start_x, cy, content_w, "  ↓", iface_rx_pct, iface_rx, colors, true);
                cy += 1;

                // Upload for this interface
                let iface_tx_pct = ((iface_tx / self.peak_tx_rate) * 100.0).min(100.0) as f32;
                self.draw_net_row(term, start_x, cy, content_w, "  ↑", iface_tx_pct, iface_tx, colors, false);
                cy += 1;
            }
        }
    }

    fn draw_net_row(&self, term: &mut Terminal, x: i32, y: i32, width: usize, label: &str, percent: f32, rate: f64, colors: &ColorState, is_download: bool) {
        // Layout: Label(10) + Meter(dynamic) + Pct(6) + Rate(12)
        let label_w = 10;
        let pct_w = 6;
        let rate_w = 12;
        let meter_w = width.saturating_sub(label_w + pct_w + rate_w);

        let mut pos = x;

        // Label
        let label_str = format!("{:<10}", label);
        term.set_str(pos, y, &label_str, Some(muted_color_scheme(colors)), false);
        pos += label_w as i32;

        // Color based on scheme
        let color = if colors.is_mono() {
            if is_download { Color::Green } else { Color::Magenta }
        } else {
            cpu_gradient_color_scheme(percent, colors)
        };

        // Meter
        if meter_w > 0 {
            draw_meter_btop_scheme(term, pos, y, meter_w, percent, colors);
            pos += meter_w as i32;
        }

        // Percentage
        let pct_str = format!("{:4.0}% ", percent);
        term.set_str(pos, y, &pct_str, Some(color), false);
        pos += pct_w as i32;

        // Rate right-aligned
        let rate_str = format_rate(rate);
        let rate_pad = rate_w.saturating_sub(rate_str.len());
        term.set_str(pos + rate_pad as i32, y, &rate_str, Some(muted_color_scheme(colors)), false);
    }
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(1.0));
    let mut monitor = NetMonitor::new();

    monitor.update(1.0)?;
    std::thread::sleep(std::time::Duration::from_millis(100));

    loop {
        if let Ok(Some((code, mods))) = term.check_key() {
            if state.handle_key(code, mods) {
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
            monitor.update(state.speed)?;
        }

        term.clear();

        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
