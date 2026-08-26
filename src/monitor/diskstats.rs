//! Readers for the two kernel files the disk panel needs beyond /proc/mounts:
//! per-device I/O counters, and the swap areas.

use std::collections::HashMap;

/// 0-based index of "milliseconds spent doing I/Os" in a /proc/diskstats row:
/// the major, minor and name tokens, then the tenth of the per-device counters.
const DISKSTATS_IO_MS: usize = 12;

/// Every active swap area folded into one figure, the way btop++ reports it.
pub(super) struct SwapInfo {
    pub total: u64,
    pub used: u64,
}

/// Sum the swap areas listed in /proc/swaps, whose Size and Used columns are in
/// KiB. A machine with no swap has no entry to draw rather than an empty one.
pub(super) fn parse_swap(content: &str) -> Option<SwapInfo> {
    let mut swap = SwapInfo { total: 0, used: 0 };

    for line in content.lines() {
        // Filename Type Size Used Priority. The header fails to parse its size
        // and is skipped along with any other malformed row.
        let parts: Vec<&str> = line.split_whitespace().collect();
        let (Some(size), Some(used)) = (parts.get(2), parts.get(3)) else {
            continue;
        };
        let (Ok(size), Ok(used)) = (size.parse::<u64>(), used.parse::<u64>()) else {
            continue;
        };
        swap.total = swap.total.saturating_add(size.saturating_mul(1024));
        swap.used = swap.used.saturating_add(used.saturating_mul(1024));
    }

    (swap.total > 0).then_some(swap)
}

/// Milliseconds each device has spent doing I/Os, keyed by its /proc/diskstats
/// name. Rows too short to carry the counter are skipped.
fn parse_diskstats_io_ms(content: &str) -> HashMap<String, u64> {
    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let name = parts.get(2)?;
            let io_ms = parts.get(DISKSTATS_IO_MS)?.parse().ok()?;
            Some((name.to_string(), io_ms))
        })
        .collect()
}

/// Share of an interval a device spent with I/O in flight.
///
/// A counter that went backwards — a device reset, or a name reused by new
/// hardware — reads as idle rather than as a negative or an absurd spike, and
/// an interval that is not a positive duration reads as idle too.
fn io_utilization(previous: u64, current: u64, elapsed_ms: f64) -> f32 {
    if !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
        return 0.0;
    }

    let busy_ms = current.saturating_sub(previous) as f64;
    ((busy_ms / elapsed_ms) * 100.0).clamp(0.0, 100.0) as f32
}

/// Per-device I/O utilization read straight from /proc/diskstats.
///
/// Deliberately not `diskio::IoMonitor`: that one skips partitions and loop
/// devices, which is exactly the set a mount point usually sits on.
pub(super) struct DeviceIo {
    /// Milliseconds spent doing I/Os as of the previous sample.
    previous: HashMap<String, u64>,
    utilization: HashMap<String, f32>,
}

impl DeviceIo {
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
            utilization: HashMap::new(),
        }
    }

    /// Fold a fresh /proc/diskstats in against the previous sample. A device
    /// seen for the first time reports 0 rather than the spike its lifetime
    /// counter would otherwise imply.
    pub fn sample(&mut self, content: &str, elapsed_ms: f64) {
        let current = parse_diskstats_io_ms(content);
        self.utilization = current
            .iter()
            .map(|(name, io_ms)| {
                let percent = match self.previous.get(name) {
                    Some(previous) => io_utilization(*previous, *io_ms, elapsed_ms),
                    None => 0.0,
                };
                (name.clone(), percent)
            })
            .collect();
        self.previous = current;
    }

    /// Forget every device, so the next sample re-establishes a baseline
    /// instead of reporting a delta that spans an outage.
    pub fn reset(&mut self) {
        self.previous.clear();
        self.utilization.clear();
    }

    /// Utilization for a mount's device, matched to a diskstats row by name.
    /// A device with no row — a network filesystem, or a mapper name the
    /// kernel reports as dm-N — has none.
    pub fn percent(&self, device: &str) -> Option<f32> {
        let name = device.strip_prefix("/dev/")?;
        self.utilization.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{io_utilization, parse_diskstats_io_ms, parse_swap, DeviceIo};

    /// A /proc/diskstats row carrying `io_ms` in the field the panel reads.
    fn diskstats_row(name: &str, io_ms: u64) -> String {
        format!("   8      18 {} 1 2 3 4 5 6 7 8 0 {} 11 0 0\n", name, io_ms)
    }

    #[test]
    fn swap_sums_every_area_and_converts_from_kib() {
        let content = concat!(
            "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n",
            "/swapfile                               file\t\t20971516\t3458424\t\t-2\n",
            "/dev/sdc3                               partition\t1048576\t\t1024\t\t-3\n",
        );

        let swap = parse_swap(content).expect("two areas");
        assert_eq!(swap.total, (20_971_516 + 1_048_576) * 1024);
        assert_eq!(swap.used, (3_458_424 + 1024) * 1024);
    }

    #[test]
    fn a_machine_without_swap_gets_no_swap_entry() {
        // Header only, an empty file, and a file that is not there at all: the
        // last reaches parse_swap as no content rather than as a zero row.
        assert!(parse_swap("Filename\tType\tSize\tUsed\tPriority\n").is_none());
        assert!(parse_swap("").is_none());
        assert!(parse_swap("garbage\n").is_none());
        // An area the kernel reports as empty is not an area to draw either.
        assert!(parse_swap("/swapfile file 0 0 -2\n").is_none());
    }

    #[test]
    fn utilization_needs_two_samples_and_never_leaves_its_range() {
        assert_eq!(io_utilization(0, 500, 1000.0), 50.0);
        assert_eq!(io_utilization(1000, 1300, 1000.0), 30.0);
        // A counter that went backwards reads as idle, not as a negative.
        assert_eq!(io_utilization(500, 400, 1000.0), 0.0);
        // Busier than the interval it is measured over, clamped to full.
        assert_eq!(io_utilization(0, 5000, 1000.0), 100.0);
        assert_eq!(io_utilization(0, 500, 0.0), 0.0);
        assert_eq!(io_utilization(0, 500, -10.0), 0.0);
        assert_eq!(io_utilization(0, 500, f64::NAN), 0.0);
    }

    #[test]
    fn the_first_sample_for_a_device_reports_zero_not_a_spike() {
        let mut io = DeviceIo::new();

        io.sample(&diskstats_row("sdb2", 29_879_106), 1000.0);
        assert_eq!(io.percent("/dev/sdb2"), Some(0.0));

        io.sample(&diskstats_row("sdb2", 29_879_406), 1000.0);
        assert_eq!(io.percent("/dev/sdb2"), Some(30.0));
    }

    #[test]
    fn attribution_covers_partitions_and_loop_backed_partitions() {
        // Exactly the devices diskio::IoMonitor filters out, which is why the
        // disk panel reads /proc/diskstats itself.
        let first = format!(
            "{}{}{}",
            diskstats_row("sdb2", 100),
            diskstats_row("loop33p1", 200),
            diskstats_row("sdb1", 300)
        );
        let second = format!(
            "{}{}{}",
            diskstats_row("sdb2", 350),
            diskstats_row("loop33p1", 700),
            diskstats_row("sdb1", 300)
        );

        let mut io = DeviceIo::new();
        io.sample(&first, 1000.0);
        io.sample(&second, 1000.0);

        assert_eq!(io.percent("/dev/sdb2"), Some(25.0));
        assert_eq!(io.percent("/dev/loop33p1"), Some(50.0));
        assert_eq!(io.percent("/dev/sdb1"), Some(0.0));
        // No row, and a device name that is not a /dev path at all.
        assert_eq!(io.percent("/dev/sdc2"), None);
        assert_eq!(io.percent("tmpfs"), None);
    }

    #[test]
    fn a_diskstats_read_failure_drops_the_baseline() {
        let mut io = DeviceIo::new();
        io.sample(&diskstats_row("sdb2", 100), 1000.0);
        io.sample(&diskstats_row("sdb2", 400), 1000.0);
        assert_eq!(io.percent("/dev/sdb2"), Some(30.0));

        io.reset();
        assert_eq!(io.percent("/dev/sdb2"), None);

        // The next sample re-establishes a baseline rather than reporting the
        // whole outage as busy time.
        io.sample(&diskstats_row("sdb2", 900_000), 1000.0);
        assert_eq!(io.percent("/dev/sdb2"), Some(0.0));
    }

    #[test]
    fn diskstats_rows_too_short_to_carry_the_counter_are_skipped() {
        let stats = concat!(
            "   8      18 sdb2 1 2 3 4 5 6 7 8 0 4200 11 0 0\n",
            "   7       0 loop0 14 0 34 0\n",
            "\n",
        );

        let parsed = parse_diskstats_io_ms(stats);
        assert_eq!(parsed.get("sdb2"), Some(&4200));
        assert_eq!(parsed.get("loop0"), None);
        assert_eq!(parsed.len(), 1);
    }
}
