use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::{MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Box, draw_meter_btop_scheme, cpu_gradient_color_scheme, format_bytes,
    text_color_scheme, muted_color_scheme, header_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::fs;
use std::io;

pub struct DiskInfo {
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
}

impl DiskInfo {
    fn percent(&self) -> f32 {
        if self.total > 0 {
            (self.used as f32 / self.total as f32) * 100.0
        } else {
            0.0
        }
    }
}

pub struct DiskMonitor {
    pub disks: Vec<DiskInfo>,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
        }
    }

    pub fn update(&mut self) -> io::Result<()> {
        self.disks.clear();

        let mounts = fs::read_to_string("/proc/mounts")?;

        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let device = parts[0];
            let mount_point = parts[1];

            if !device.starts_with("/dev/") {
                continue;
            }
            if mount_point.starts_with("/snap") || mount_point.starts_with("/boot/efi") {
                continue;
            }

            if let Ok(statvfs) = Self::statvfs(mount_point) {
                let total = statvfs.blocks * statvfs.frsize;
                let free = statvfs.bfree * statvfs.frsize;
                let used = total.saturating_sub(free);

                if total > 0 {
                    self.disks.push(DiskInfo {
                        mount_point: mount_point.to_string(),
                        total,
                        used,
                    });
                }
            }
        }

        self.disks.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        Ok(())
    }

    fn statvfs(path: &str) -> io::Result<StatVfs> {
        use std::mem::MaybeUninit;
        use std::ffi::CString;

        let c_path = CString::new(path).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid path"))?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

        let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };

        if result == 0 {
            let stat = unsafe { stat.assume_init() };
            Ok(StatVfs {
                frsize: stat.f_frsize as u64,
                blocks: stat.f_blocks as u64,
                bfree: stat.f_bfree as u64,
            })
        } else {
            Err(io::Error::last_os_error())
        }
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

        if self.disks.is_empty() {
            let cy = y + (h as i32 / 2);
            term.set_str(x, cy, "No disks found", Some(Color::Yellow), false);
            return;
        }

        // Calculate total storage
        let total_size: u64 = self.disks.iter().map(|d| d.total).sum();
        let total_used: u64 = self.disks.iter().map(|d| d.used).sum();

        // Panel height: Title(1) + one row per disk
        let max_disks = (h - 1).min(self.disks.len());
        let panel_height = 1 + max_disks;

        // Vertically center
        let start_y = y + ((h as i32 - panel_height as i32) / 2).max(0);
        let mut cy = start_y;

        // Title with total storage
        term.set_str(x, cy, "Disks", Some(text_color_scheme(colors)), true);
        let total_str = format!("{}/{}", format_bytes(total_used), format_bytes(total_size));
        term.set_str(x + w as i32 - total_str.len() as i32, cy, &total_str, Some(muted_color_scheme(colors)), false);
        cy += 1;

        // Each disk
        for disk in self.disks.iter().take(max_disks) {
            let pct = disk.percent();
            let size_str = format!("{}/{}", format_bytes(disk.used), format_bytes(disk.total));
            self.draw_disk_row(term, x, cy, w, &disk.mount_point, pct, &size_str, colors);
            cy += 1;
        }

        // Show "+N more" if there are more disks
        if self.disks.len() > max_disks {
            let remaining = self.disks.len() - max_disks;
            let msg = format!("+{} more", remaining);
            term.set_str(x + w as i32 - msg.len() as i32, cy - 1, &msg, Some(muted_color_scheme(colors)), false);
        }
    }

    fn draw_disk_row(&self, term: &mut Terminal, x: i32, y: i32, width: usize, mount: &str, percent: f32, size_str: &str, colors: &ColorState) {
        // Layout: Mount(12) + Meter(dynamic) + Pct(6) + Size(18)
        let mount_w = 12;
        let pct_w = 6;
        let size_w = 18;
        let meter_w = width.saturating_sub(mount_w + pct_w + size_w);

        let mut pos = x;

        // Mount point (truncated if needed)
        let mount_display: String = if mount.len() <= mount_w - 1 {
            format!("{:<width$}", mount, width = mount_w)
        } else if mount == "/" {
            format!("{:<width$}", "/", width = mount_w)
        } else {
            // Show last component
            let short = mount.split('/').last().unwrap_or("?");
            if short.len() <= mount_w - 1 {
                format!("{:<width$}", short, width = mount_w)
            } else {
                format!("{:<width$}", &short[..mount_w - 1], width = mount_w)
            }
        };
        term.set_str(pos, y, &mount_display, Some(header_color_scheme(colors)), false);
        pos += mount_w as i32;

        let color = cpu_gradient_color_scheme(percent, colors);

        // Meter
        if meter_w > 0 {
            draw_meter_btop_scheme(term, pos, y, meter_w, percent, colors);
            pos += meter_w as i32;
        }

        // Percentage
        let pct_str = format!("{:4.0}% ", percent);
        term.set_str(pos, y, &pct_str, Some(color), false);
        pos += pct_w as i32;

        // Size right-aligned
        let size_pad = size_w.saturating_sub(size_str.len());
        term.set_str(pos + size_pad as i32, y, size_str, Some(muted_color_scheme(colors)), false);
    }
}

struct StatVfs {
    frsize: u64,
    blocks: u64,
    bfree: u64,
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(2.0));
    let mut monitor = DiskMonitor::new();

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
            monitor.update()?;
        }

        term.clear();

        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
