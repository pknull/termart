use crate::colors::ColorState;
use crate::help::HelpSpec;
use crate::monitor::layout::{
    cpu_gradient_color_scheme, draw_history_graph_scheme, draw_meter_btop_scheme, format_bytes,
    header_color_scheme, headroom_gradient_color_scheme, muted_color_scheme, split_row_for_graph,
    text_color_scheme, Rect, SampleHistory, HISTORY_CAPACITY,
};
use crate::monitor::{MonitorAction, MonitorConfig, MonitorState};
use crate::terminal::Terminal;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::collections::HashMap;
use std::fs;
use std::io;

fn decode_mount_field(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1..=index + 3]
                .iter()
                .all(|byte| matches!(byte, b'0'..=b'7'))
        {
            let value = (bytes[index + 1] - b'0') * 64
                + (bytes[index + 2] - b'0') * 8
                + (bytes[index + 3] - b'0');
            decoded.push(value);
            index += 4;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub struct DiskInfo {
    pub device: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
}

impl DiskInfo {
    fn percent(&self) -> f32 {
        let usable = self.used.saturating_add(self.available);
        if usable > 0 {
            (self.used as f32 / usable as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Share of usable capacity still available to the user. The history graph
    /// reads this as headroom, so a falling graph is a filling disk.
    fn available_percent(&self) -> f32 {
        100.0 - self.percent()
    }
}

/// A mount point's availability history plus the device it was recorded from,
/// so a different filesystem mounted at the same path starts a fresh graph
/// instead of inheriting samples that described the old one.
struct MountHistory {
    device: String,
    samples: SampleHistory,
}

impl MountHistory {
    fn new(device: &str) -> Self {
        Self {
            device: device.to_string(),
            samples: SampleHistory::new(HISTORY_CAPACITY),
        }
    }
}

pub struct DiskMonitor {
    pub disks: Vec<DiskInfo>,
    histories: HashMap<String, MountHistory>,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
            histories: HashMap::new(),
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
            let mount_point = decode_mount_field(parts[1]);

            if !device.starts_with("/dev/") {
                continue;
            }
            if mount_point.starts_with("/snap") || mount_point.starts_with("/boot/efi") {
                continue;
            }

            if let Ok(statvfs) = Self::statvfs(&mount_point) {
                let total = statvfs.blocks * statvfs.frsize;
                let free = statvfs.bfree * statvfs.frsize;
                let available = statvfs.bavail * statvfs.frsize;
                let used = total.saturating_sub(free);

                if total > 0 {
                    self.disks.push(DiskInfo {
                        device: device.to_string(),
                        mount_point,
                        total,
                        used,
                        available,
                    });
                }
            }
        }

        self.disks.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        self.record_history();
        Ok(())
    }

    /// Record each mount point's current headroom exactly once per refresh,
    /// first forgetting mounts that have gone away so the map cannot grow
    /// without bound across remounts.
    ///
    /// When /proc/mounts lists a mount point more than once (an overmount or a
    /// bind remount) the last row wins, matching the filesystem statvfs sees;
    /// pushing every row would scroll that graph at double speed. A history
    /// whose device changes is restarted rather than continued: its samples
    /// described a filesystem that is no longer there.
    fn record_history(&mut self) {
        let disks = &self.disks;
        self.histories
            .retain(|mount, _| disks.iter().any(|disk| &disk.mount_point == mount));

        let mut visible: HashMap<&str, &DiskInfo> = HashMap::new();
        for disk in &self.disks {
            visible.insert(disk.mount_point.as_str(), disk);
        }

        for disk in visible.into_values() {
            let entry = self
                .histories
                .entry(disk.mount_point.clone())
                .or_insert_with(|| MountHistory::new(&disk.device));
            if entry.device != disk.device {
                *entry = MountHistory::new(&disk.device);
            }
            entry.samples.push(disk.available_percent());
        }
    }

    fn statvfs(path: &str) -> io::Result<StatVfs> {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid path"))?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

        let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };

        if result == 0 {
            let stat = unsafe { stat.assume_init() };
            Ok(StatVfs {
                frsize: stat.f_frsize,
                blocks: stat.f_blocks,
                bfree: stat.f_bfree,
                bavail: stat.f_bavail,
            })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[allow(dead_code)]
    pub fn render(&self, term: &mut Terminal, bx: &Rect, colors: &ColorState) {
        let x = bx.inner_x();
        let y = bx.inner_y();
        let w = bx.inner_width() as usize;
        let h = bx.inner_height() as usize;
        self.render_at(term, x, y, w, h, colors);
    }

    pub fn render_fullscreen(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        self.render_at(term, 0, 0, w, h, colors);
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
        if h < 2 || w < 30 {
            return;
        }

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
        term.set_str(
            x + w as i32 - total_str.len() as i32,
            cy,
            &total_str,
            Some(muted_color_scheme(colors)),
            false,
        );
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
            term.set_str(
                x + w as i32 - msg.len() as i32,
                cy - 1,
                &msg,
                Some(muted_color_scheme(colors)),
                false,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_disk_row(
        &self,
        term: &mut Terminal,
        x: i32,
        y: i32,
        width: usize,
        mount: &str,
        percent: f32,
        size_str: &str,
        colors: &ColorState,
    ) {
        // Layout: Mount(12) + Meter(dynamic) + Pct(6) + Size(18)
        let mount_w = 12;
        let pct_w = 6;
        let size_w = 18;
        let elastic = width.saturating_sub(mount_w + pct_w + size_w);

        let mut pos = x;

        // Mount point (truncated if needed)
        let mount_display: String = if mount.len() < mount_w {
            format!("{:<width$}", mount, width = mount_w)
        } else if mount == "/" {
            format!("{:<width$}", "/", width = mount_w)
        } else {
            // Show last component
            let short = mount.split('/').next_back().unwrap_or("?");
            if short.len() < mount_w {
                format!("{:<width$}", short, width = mount_w)
            } else {
                // Truncate by chars, not bytes, to avoid panicking on a multibyte boundary.
                let truncated: String = short.chars().take(mount_w.saturating_sub(1)).collect();
                format!("{:<width$}", truncated, width = mount_w)
            }
        };
        term.set_str(
            pos,
            y,
            &mount_display,
            Some(header_color_scheme(colors)),
            false,
        );
        pos += mount_w as i32;

        let color = cpu_gradient_color_scheme(percent, colors);

        // Meter, plus a history graph when the row can afford one.
        self.draw_disk_span(term, pos, y, elastic, percent, mount, colors);
        pos += elastic as i32;

        // Percentage
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

    /// Draw the elastic part of a row: the usage meter, and a right-anchored
    /// graph of the mount's availability history when the row is wide enough to
    /// add one without shrinking the meter past its floor.
    #[allow(clippy::too_many_arguments)]
    fn draw_disk_span(
        &self,
        term: &mut Terminal,
        x: i32,
        y: i32,
        elastic: usize,
        percent: f32,
        mount: &str,
        colors: &ColorState,
    ) {
        let history = self.histories.get(mount).map(|entry| &entry.samples);
        let (meter_w, graph_w) = match history {
            Some(_) => split_row_for_graph(elastic),
            None => (elastic, 0),
        };

        if meter_w > 0 {
            draw_meter_btop_scheme(term, x, y, meter_w, percent, colors);
        }

        if let Some(history) = history {
            draw_history_graph_scheme(
                term,
                x + (elastic - graph_w) as i32,
                y,
                graph_w,
                history,
                headroom_gradient_color_scheme,
                colors,
            );
        }
    }
}

struct StatVfs {
    frsize: u64,
    blocks: u64,
    bfree: u64,
    bavail: u64,
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step, 2.0);
    let mut monitor = DiskMonitor::new();
    const HELP: HelpSpec = HelpSpec::monitor("DISK MONITOR", &[]);

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
    use super::{decode_mount_field, DiskInfo, DiskMonitor};

    #[test]
    fn mount_fields_decode_proc_octal_escapes() {
        assert_eq!(decode_mount_field(r"/media/My\040Disk"), "/media/My Disk");
        assert_eq!(
            decode_mount_field(r"/path/with\134slash"),
            r"/path/with\slash"
        );
        assert_eq!(
            decode_mount_field(r"/tabs\011and\012lines"),
            "/tabs\tand\nlines"
        );
        assert_eq!(decode_mount_field(r"/incomplete\04"), r"/incomplete\04");
    }

    fn disk(device: &str, mount_point: &str, used: u64, available: u64) -> DiskInfo {
        DiskInfo {
            device: device.to_string(),
            mount_point: mount_point.to_string(),
            total: used + available,
            used,
            available,
        }
    }

    #[test]
    fn capacity_percentage_uses_user_available_space() {
        let disk = DiskInfo {
            device: "/dev/sda1".to_string(),
            mount_point: "/".to_string(),
            total: 1000,
            used: 800,
            available: 100,
        };

        assert!((disk.percent() - 88.888_89).abs() < 0.001);
        assert!((disk.available_percent() - 11.111_11).abs() < 0.001);
    }

    #[test]
    fn history_records_headroom_and_forgets_removed_mounts() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![
            disk("/dev/sda1", "/", 750, 250),
            disk("/dev/sdb1", "/data", 100, 900),
        ];

        monitor.record_history();
        monitor.record_history();

        assert_eq!(monitor.histories.len(), 2);
        let root = &monitor.histories["/"].samples;
        assert_eq!(root.len(), 2);
        assert!((root.iter().last().unwrap() - 25.0).abs() < 0.001);

        monitor.disks.remove(1);
        monitor.record_history();

        assert_eq!(monitor.histories.len(), 1);
        assert!(monitor.histories.contains_key("/"));
    }

    #[test]
    fn duplicate_mount_rows_push_one_sample_from_the_last_row() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![
            disk("/dev/sda1", "/mnt", 900, 100),
            disk("/dev/sdb1", "/mnt", 250, 750),
        ];

        monitor.record_history();

        let history = &monitor.histories["/mnt"];
        assert_eq!(history.samples.len(), 1);
        assert_eq!(history.device, "/dev/sdb1");
        assert!((history.samples.iter().last().unwrap() - 75.0).abs() < 0.001);
    }

    #[test]
    fn swapping_the_device_at_a_mount_point_restarts_its_history() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![disk("/dev/sda1", "/mnt", 900, 100)];
        monitor.record_history();
        monitor.record_history();
        assert_eq!(monitor.histories["/mnt"].samples.len(), 2);

        monitor.disks = vec![disk("/dev/sdb1", "/mnt", 500, 500)];
        monitor.record_history();

        let history = &monitor.histories["/mnt"];
        assert_eq!(history.device, "/dev/sdb1");
        assert_eq!(history.samples.len(), 1);
        assert!((history.samples.iter().last().unwrap() - 50.0).abs() < 0.001);
    }
}
