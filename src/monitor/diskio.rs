use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::{MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Box, draw_meter_btop_scheme, format_rate, format_bytes,
    cpu_gradient_color_scheme, text_color_scheme, muted_color_scheme, header_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::fs;
use std::io;

#[derive(Clone)]
struct DiskStats {
    name: String,
    read_bytes: u64,
    write_bytes: u64,
    read_rate: f64,
    write_rate: f64,
    prev_read_bytes: u64,
    prev_write_bytes: u64,
}

pub struct IoMonitor {
    disks: Vec<DiskStats>,
    pub total_read_rate: f64,
    pub total_write_rate: f64,
    pub peak_read_rate: f64,
    pub peak_write_rate: f64,
}

impl IoMonitor {
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
            total_read_rate: 0.0,
            total_write_rate: 0.0,
            peak_read_rate: 100.0 * 1024.0 * 1024.0, // Start with 100MB/s as minimum scale
            peak_write_rate: 100.0 * 1024.0 * 1024.0,
        }
    }

    pub fn update(&mut self, interval: f32) -> io::Result<()> {
        let content = fs::read_to_string("/proc/diskstats")?;
        let mut new_disks = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 14 {
                continue;
            }

            let name = parts[2];
            // Skip virtual/loop devices
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
                continue;
            }
            // Skip partitions - only show whole disks
            // For sd/hd devices: skip if ends with digit (sda1, sdb2)
            // For nvme devices: skip if contains 'p' partition marker (nvme0n1p1)
            if (name.starts_with("sd") || name.starts_with("hd")) &&
               name.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                continue;
            }
            if name.starts_with("nvme") && name.contains('p') {
                continue;
            }

            let sectors_read: u64 = parts[5].parse().unwrap_or(0);
            let sectors_written: u64 = parts[9].parse().unwrap_or(0);
            let read_bytes = sectors_read * 512;
            let write_bytes = sectors_written * 512;

            // Skip disks with no activity
            if read_bytes == 0 && write_bytes == 0 {
                continue;
            }

            // Find existing disk or create new
            let mut disk = if let Some(existing) = self.disks.iter()
                .find(|d| d.name == name)
                .cloned() {
                existing
            } else {
                DiskStats {
                    name: name.to_string(),
                    read_bytes: 0,
                    write_bytes: 0,
                    read_rate: 0.0,
                    write_rate: 0.0,
                    prev_read_bytes: read_bytes, // Start with current for new disks
                    prev_write_bytes: write_bytes,
                }
            };

            // Calculate rates
            let read_diff = read_bytes.saturating_sub(disk.prev_read_bytes);
            let write_diff = write_bytes.saturating_sub(disk.prev_write_bytes);
            disk.read_rate = read_diff as f64 / interval as f64;
            disk.write_rate = write_diff as f64 / interval as f64;

            // Update stats
            disk.prev_read_bytes = disk.read_bytes;
            disk.prev_write_bytes = disk.write_bytes;
            disk.read_bytes = read_bytes;
            disk.write_bytes = write_bytes;

            new_disks.push(disk);
        }

        // Calculate totals
        self.total_read_rate = 0.0;
        self.total_write_rate = 0.0;
        for disk in &new_disks {
            self.total_read_rate += disk.read_rate;
            self.total_write_rate += disk.write_rate;
        }

        self.disks = new_disks;

        // Update peak rates for auto-scaling (with decay)
        if self.total_read_rate > self.peak_read_rate {
            self.peak_read_rate = self.total_read_rate;
        } else {
            self.peak_read_rate = (self.peak_read_rate * 0.999).max(100.0 * 1024.0 * 1024.0);
        }
        if self.total_write_rate > self.peak_write_rate {
            self.peak_write_rate = self.total_write_rate;
        } else {
            self.peak_write_rate = (self.peak_write_rate * 0.999).max(100.0 * 1024.0 * 1024.0);
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

        let num_disks = self.disks.len().min(6);
        if num_disks == 0 {
            let cy = y + (h as i32 / 2);
            term.set_str(x, cy, "No disks found", Some(Color::Yellow), false);
            return;
        }

        // Calculate panel height: Title(1) + per-disk lines (name + R + W = 3 per disk)
        let panel_height = 1 + num_disks * 3;

        // Vertically center
        let start_y = y + ((h as i32 - panel_height as i32) / 2).max(0);
        let mut cy = start_y;

        // Title with total transferred
        let total_read: u64 = self.disks.iter().map(|d| d.read_bytes).sum();
        let total_write: u64 = self.disks.iter().map(|d| d.write_bytes).sum();
        term.set_str(x, cy, "Disk I/O", Some(text_color_scheme(colors)), true);
        let totals_str = format!("R:{} W:{}", format_bytes(total_read), format_bytes(total_write));
        term.set_str(x + w as i32 - totals_str.len() as i32, cy, &totals_str, Some(muted_color_scheme(colors)), false);
        cy += 1;

        // Per-disk breakdown
        for disk in self.disks.iter().take(num_disks) {
            // Disk name as label
            term.set_str(x, cy, &disk.name, Some(header_color_scheme(colors)), false);
            cy += 1;

            // Read for this disk
            let disk_read_pct = ((disk.read_rate / self.peak_read_rate) * 100.0).min(100.0) as f32;
            self.draw_io_row(term, x, cy, w, "  Read", disk_read_pct, disk.read_rate, colors, true);
            cy += 1;

            // Write for this disk
            let disk_write_pct = ((disk.write_rate / self.peak_write_rate) * 100.0).min(100.0) as f32;
            self.draw_io_row(term, x, cy, w, "  Write", disk_write_pct, disk.write_rate, colors, false);
            cy += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_io_row(&self, term: &mut Terminal, x: i32, y: i32, width: usize, label: &str, percent: f32, rate: f64, colors: &ColorState, is_read: bool) {
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
            if is_read { Color::Green } else { Color::Magenta }
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
    let mut monitor = IoMonitor::new();

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
