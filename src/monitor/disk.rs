use crate::colors::ColorState;
use crate::help::HelpSpec;
use crate::monitor::diskpanel::{draw_entry, entry_name, plan_render, Entry};
use crate::monitor::diskstats::{parse_swap, DeviceIo, SwapInfo};
use crate::monitor::layout::{
    format_bytes, muted_color_scheme, text_color_scheme, Rect, SampleHistory, HISTORY_CAPACITY,
};
use crate::monitor::{MonitorAction, MonitorConfig, MonitorState};
use crate::terminal::Terminal;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::Instant;

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

/// Mounts the panel reports on: real block devices, minus the snap loopbacks
/// that would otherwise bury every filesystem an operator cares about. The EFI
/// system partition stays — it is small, it fills quietly, and a full one
/// breaks the next kernel update.
fn is_rendered_mount(device: &str, mount_point: &str) -> bool {
    device.starts_with("/dev/") && !mount_point.starts_with("/snap")
}

pub struct DiskInfo {
    pub device: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
}

/// A mount point's I/O history plus the device it was recorded from, so a
/// different filesystem mounted at the same path starts a fresh graph instead
/// of inheriting samples that described the old one.
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
    swap: Option<SwapInfo>,
    io: DeviceIo,
    histories: HashMap<String, MountHistory>,
    last_io_sample: Option<Instant>,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
            swap: None,
            io: DeviceIo::new(),
            histories: HashMap::new(),
            last_io_sample: None,
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

            if !is_rendered_mount(device, &mount_point) {
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
        // Swap is optional on any machine and absent in a container, so a read
        // failure means no swap entry rather than a failed refresh.
        self.swap = fs::read_to_string("/proc/swaps")
            .ok()
            .as_deref()
            .and_then(parse_swap);
        self.sample_io();
        self.record_history();
        Ok(())
    }

    /// Sample /proc/diskstats against the wall clock since the previous
    /// sample, which is what the utilization is a share of.
    fn sample_io(&mut self) {
        let now = Instant::now();
        let elapsed_ms = self
            .last_io_sample
            .map(|at| (now - at).as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        self.last_io_sample = Some(now);

        match fs::read_to_string("/proc/diskstats") {
            Ok(content) => self.io.sample(&content, elapsed_ms),
            Err(_) => self.io.reset(),
        }
    }

    /// Record each mount point's current I/O utilization exactly once per
    /// refresh, first forgetting mounts that have gone away so the map cannot
    /// grow without bound across remounts.
    ///
    /// When /proc/mounts lists a mount point more than once (an overmount or a
    /// bind remount) the last row wins, matching the filesystem statvfs sees;
    /// pushing every row would scroll that graph at double speed. A history
    /// whose device changes is restarted rather than continued: its samples
    /// described a device that is no longer there.
    fn record_history(&mut self) {
        let disks = &self.disks;
        let io = &self.io;
        let histories = &mut self.histories;

        histories.retain(|mount, _| disks.iter().any(|disk| &disk.mount_point == mount));

        let mut visible: HashMap<&str, &DiskInfo> = HashMap::new();
        for disk in disks {
            visible.insert(disk.mount_point.as_str(), disk);
        }

        for disk in visible.into_values() {
            let Some(percent) = io.percent(&disk.device) else {
                continue;
            };
            let entry = histories
                .entry(disk.mount_point.clone())
                .or_insert_with(|| MountHistory::new(&disk.device));
            if entry.device != disk.device {
                *entry = MountHistory::new(&disk.device);
            }
            entry.samples.push(percent);
        }
    }

    /// The panel's entries in render order: every mount point, with swap folded
    /// in after the first of them, which is where btop++ puts it.
    fn entries(&self) -> Vec<Entry<'_>> {
        let mut entries: Vec<Entry<'_>> = self
            .disks
            .iter()
            .map(|disk| Entry {
                name: entry_name(&disk.mount_point).to_string(),
                total: disk.total,
                used: disk.used,
                available: disk.available,
                io: self.mount_io(disk),
            })
            .collect();

        if let Some(swap) = &self.swap {
            let entry = Entry {
                name: "swap".to_string(),
                total: swap.total,
                used: swap.used,
                available: swap.total.saturating_sub(swap.used),
                io: None,
            };
            entries.insert(entries.len().min(1), entry);
        }

        entries
    }

    /// A mount's utilization and its history, present only once its device has
    /// a diskstats row and a sample has been recorded against it.
    fn mount_io(&self, disk: &DiskInfo) -> Option<(f32, &SampleHistory)> {
        let percent = self.io.percent(&disk.device)?;
        let history = self.histories.get(&disk.mount_point)?;
        Some((percent, &history.samples))
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

        let entries = self.entries();
        if entries.is_empty() {
            let cy = y + (h as i32 / 2);
            term.set_str(x, cy, "No disks found", Some(Color::Yellow), false);
            return;
        }

        let (plan, geom) = plan_render(&entries, w, h.saturating_sub(1));

        // Title(1) plus the rows the plan affords, vertically centered.
        let panel_height = 1 + plan.rows;
        let mut cy = y + ((h as i32 - panel_height as i32) / 2).max(0);
        self.draw_title(term, x, cy, w, colors);
        cy += 1;

        for (index, entry) in entries.iter().take(plan.entries).enumerate() {
            if index > 0 && plan.spacers {
                cy += 1;
            }
            cy = draw_entry(term, x, cy, entry, geom, colors);
        }

        if plan.affordance {
            let msg = format!("+{} more", entries.len() - plan.entries);
            term.set_str(
                x + w as i32 - msg.chars().count() as i32,
                cy,
                &msg,
                Some(muted_color_scheme(colors)),
                false,
            );
        }
    }

    /// Draw the panel title and its grand total. The total stays a filesystem
    /// figure: a swap file lives inside a mount already counted here, so
    /// folding swap in would count the same bytes twice.
    fn draw_title(&self, term: &mut Terminal, x: i32, y: i32, w: usize, colors: &ColorState) {
        let total_size: u64 = self.disks.iter().map(|disk| disk.total).sum();
        let total_used: u64 = self.disks.iter().map(|disk| disk.used).sum();

        term.set_str(x, y, "Disks", Some(text_color_scheme(colors)), true);
        let total_str = format!("{}/{}", format_bytes(total_used), format_bytes(total_size));
        term.set_str(
            x + w as i32 - total_str.chars().count() as i32,
            y,
            &total_str,
            Some(muted_color_scheme(colors)),
            false,
        );
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
    use super::{decode_mount_field, is_rendered_mount, DiskInfo, DiskMonitor, SwapInfo};

    /// A /proc/diskstats row carrying `io_ms` in the field the panel reads.
    fn diskstats_row(name: &str, io_ms: u64) -> String {
        format!("   8      18 {} 1 2 3 4 5 6 7 8 0 {} 11 0 0\n", name, io_ms)
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

    #[test]
    fn the_efi_partition_is_kept_and_the_snap_loopbacks_are_not() {
        assert!(is_rendered_mount("/dev/sdb1", "/boot/efi"));
        assert!(is_rendered_mount("/dev/sdb2", "/"));
        assert!(!is_rendered_mount("/dev/loop3", "/snap/core/17284"));
        assert!(!is_rendered_mount("tmpfs", "/run"));
    }

    #[test]
    fn swap_renders_after_the_first_mount_with_its_own_free_space() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![
            disk("/dev/sdb2", "/", 750, 250),
            disk("/dev/sdb3", "/home", 100, 900),
        ];
        monitor.swap = Some(SwapInfo {
            total: 2048,
            used: 512,
        });

        let entries = monitor.entries();
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["root", "swap", "home"]);

        let swap = &entries[1];
        assert_eq!(swap.available, 1536);
        assert!((swap.percent() - 25.0).abs() < 0.001);
        assert!(swap.io.is_none(), "swap has no backing diskstats device");
    }

    #[test]
    fn swap_is_the_only_entry_when_nothing_is_mounted() {
        let mut monitor = DiskMonitor::new();
        monitor.swap = Some(SwapInfo {
            total: 2048,
            used: 512,
        });

        let entries = monitor.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "swap");
    }

    #[test]
    fn only_mounts_with_a_diskstats_row_get_an_io_line() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![
            disk("/dev/sdb2", "/", 750, 250),
            disk("/dev/nfs0", "/net", 100, 900),
        ];
        monitor.io.sample(&diskstats_row("sdb2", 100), 1000.0);
        monitor.io.sample(&diskstats_row("sdb2", 500), 1000.0);
        monitor.record_history();

        let entries = monitor.entries();
        let root = entries.iter().find(|e| e.name == "root").expect("root");
        let net = entries.iter().find(|e| e.name == "net").expect("net");

        let (percent, history) = root.io.expect("sdb2 has a diskstats row");
        assert!((percent - 40.0).abs() < 0.001);
        assert_eq!(history.len(), 1);
        assert!(net.io.is_none(), "no diskstats row means no IO line");
    }

    #[test]
    fn history_records_utilization_and_forgets_removed_mounts() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![
            disk("/dev/sdb2", "/", 750, 250),
            disk("/dev/sdb3", "/data", 100, 900),
        ];
        let stats = format!("{}{}", diskstats_row("sdb2", 0), diskstats_row("sdb3", 0));
        monitor.io.sample(&stats, 1000.0);
        monitor.record_history();

        let stats = format!("{}{}", diskstats_row("sdb2", 250), diskstats_row("sdb3", 0));
        monitor.io.sample(&stats, 1000.0);
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
        let stats = format!("{}{}", diskstats_row("sda1", 0), diskstats_row("sdb1", 0));
        monitor.io.sample(&stats, 1000.0);
        monitor.record_history();

        let history = &monitor.histories["/mnt"];
        assert_eq!(history.samples.len(), 1);
        assert_eq!(history.device, "/dev/sdb1");
    }

    #[test]
    fn swapping_the_device_at_a_mount_point_restarts_its_history() {
        let mut monitor = DiskMonitor::new();
        monitor.disks = vec![disk("/dev/sda1", "/mnt", 900, 100)];
        let stats = format!("{}{}", diskstats_row("sda1", 0), diskstats_row("sdb1", 0));
        monitor.io.sample(&stats, 1000.0);
        monitor.record_history();
        monitor.record_history();
        assert_eq!(monitor.histories["/mnt"].samples.len(), 2);

        monitor.disks = vec![disk("/dev/sdb1", "/mnt", 500, 500)];
        monitor.record_history();

        let history = &monitor.histories["/mnt"];
        assert_eq!(history.device, "/dev/sdb1");
        assert_eq!(history.samples.len(), 1);
    }
}
