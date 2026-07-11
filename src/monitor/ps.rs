//! Process list monitor - shows top processes by CPU/memory usage

use crate::colors::ColorState;
use crate::help::{render_help_spec, HelpEntry, HelpSpec};
use crate::monitor::layout::{cpu_gradient_color_scheme, muted_color_scheme, text_color_scheme};
use crate::monitor::MonitorState;
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::terminal::size;
use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Clone)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_pct: f32,
    mem_pct: f32,
    cpu_ticks: u64, // Raw ticks for delta calculation
    is_kernel: bool,
}

pub struct PsMonitor {
    processes: Vec<ProcessInfo>,
    prev_ticks: HashMap<u32, (u64, f64)>, // PID -> (cpu_ticks, uptime)
    mem_total: u64,
    sort_by_mem: bool,
    show_kernel: bool,
    clock_ticks: f64,
    page_size: u64,
}

impl PsMonitor {
    pub fn new(show_kernel: bool) -> Self {
        Self {
            processes: Vec::new(),
            prev_ticks: HashMap::new(),
            mem_total: get_mem_total().unwrap_or(1),
            sort_by_mem: false,
            show_kernel,
            clock_ticks: sysconf_value(libc::_SC_CLK_TCK).unwrap_or(100) as f64,
            page_size: sysconf_value(libc::_SC_PAGESIZE).unwrap_or(4096),
        }
    }

    pub fn toggle_sort(&mut self) {
        self.sort_by_mem = !self.sort_by_mem;
        // Re-sort immediately
        if self.sort_by_mem {
            self.processes.sort_by(|a, b| {
                b.mem_pct
                    .partial_cmp(&a.mem_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            self.processes.sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    pub fn update(&mut self) -> io::Result<()> {
        let uptime = get_uptime_secs()?;
        let mut new_processes = Vec::new();
        let mut new_ticks = HashMap::new();

        // Read /proc directory for PIDs
        let proc_dir = fs::read_dir("/proc")?;

        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only process numeric directories (PIDs)
            if let Ok(pid) = name_str.parse::<u32>() {
                if let Some(info) = self.read_process(pid, uptime) {
                    // Filter kernel threads (empty cmdline) unless show_kernel
                    if self.show_kernel || !info.is_kernel {
                        new_ticks.insert(pid, (info.cpu_ticks, uptime)); // Store raw ticks for next delta
                        new_processes.push(info);
                    }
                }
            }
        }

        // Sort by CPU% or MEM%
        if self.sort_by_mem {
            new_processes.sort_by(|a, b| {
                b.mem_pct
                    .partial_cmp(&a.mem_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            new_processes.sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        self.processes = new_processes;
        self.prev_ticks = new_ticks;
        Ok(())
    }

    fn read_process(&self, pid: u32, uptime: f64) -> Option<ProcessInfo> {
        let stat_path = format!("/proc/{}/stat", pid);
        let stat_content = fs::read_to_string(&stat_path).ok()?;

        // Parse stat file - command name is in parentheses and may contain spaces
        // Format: pid (comm) state ppid ...
        let open_paren = stat_content.find('(')?;
        let close_paren = stat_content.rfind(')')?;

        let name = stat_content[open_paren + 1..close_paren].to_string();
        let rest = &stat_content[close_paren + 2..]; // Skip ") "
        let fields: Vec<&str> = rest.split_whitespace().collect();

        if fields.len() < 22 {
            return None;
        }

        let utime: u64 = fields[11].parse().ok()?; // Field 14 in original (0-indexed after comm: 11)
        let stime: u64 = fields[12].parse().ok()?; // Field 15
        let rss_pages: u64 = fields[21].parse().ok()?; // Field 24 (RSS in pages)

        let cpu_ticks = utime + stime;
        let rss_bytes = rss_pages.saturating_mul(self.page_size);

        // Calculate CPU% using delta from previous reading
        let cpu_pct = if let Some(&(prev_ticks, prev_uptime)) = self.prev_ticks.get(&pid) {
            let delta_ticks = cpu_ticks.saturating_sub(prev_ticks);
            let delta_time = uptime - prev_uptime;
            if delta_time > 0.0 {
                ((delta_ticks as f64 / self.clock_ticks) / delta_time * 100.0) as f32
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Calculate MEM%
        let mem_pct = (rss_bytes as f64 / self.mem_total as f64 * 100.0) as f32;

        // Get better command name from cmdline if available
        let cmdline_name = get_cmdline(pid).unwrap_or_default();
        let is_kernel = cmdline_name.is_empty();
        let display_name = if cmdline_name.is_empty() {
            format!("[{}]", name) // Kernel thread
        } else {
            cmdline_name
        };

        Some(ProcessInfo {
            pid,
            name: display_name,
            cpu_pct,
            mem_pct,
            cpu_ticks,
            is_kernel,
        })
    }

    pub fn render(
        &self,
        term: &mut Terminal,
        w: usize,
        h: usize,
        colors: &ColorState,
        max_procs: usize,
    ) {
        if h < 3 || w < 40 {
            return;
        }

        let header_y = 0;
        let mut y = 1;

        // Header: PID, CPU%, MEM%, PROCESS
        let sort_indicator = if self.sort_by_mem { "MEM%" } else { "CPU%" };
        let header = format!("{:>7}  {:>6}  {:>6}  {}", "PID", "CPU%", "MEM%", "PROCESS");
        let header_truncated: String = header.chars().take(w).collect();
        term.set_str(
            0,
            header_y,
            &header_truncated,
            Some(text_color_scheme(colors)),
            true,
        );

        // Sort indicator at top right
        let sort_hint = format!("[m]Sort:{}", sort_indicator);
        if w > sort_hint.len() + 2 {
            term.set_str(
                (w - sort_hint.len()) as i32,
                header_y,
                &sort_hint,
                Some(muted_color_scheme(colors)),
                false,
            );
        }

        // Process rows
        let available_rows = h.saturating_sub(2); // Reserve header + hint line
        let show_count = self.processes.len().min(max_procs).min(available_rows);

        for proc in self.processes.iter().take(show_count) {
            // Format the row: PID, CPU%, MEM%, PROCESS
            let row = format!(
                "{:>7}  {:>5.1}%  {:>5.1}%  {}",
                proc.pid, proc.cpu_pct, proc.mem_pct, proc.name
            );

            // Truncate to terminal width
            let row_truncated: String = row.chars().take(w).collect();

            // Color based on CPU usage
            let row_color = cpu_gradient_color_scheme(proc.cpu_pct.min(100.0), colors);
            term.set_str(0, y, &row_truncated, Some(row_color), false);

            y += 1;
            if y >= h as i32 - 1 {
                break;
            }
        }

        // Hint line at bottom
        let hint = "q:Quit  m:Sort  Space:Pause  1-9:Speed  ?:Help";
        let hint_y = (h - 1) as i32;
        term.set_str(0, hint_y, hint, Some(muted_color_scheme(colors)), false);
    }
}

fn sysconf_value(name: libc::c_int) -> Option<u64> {
    let value = unsafe { libc::sysconf(name) };
    (value > 0).then_some(value as u64)
}

fn get_uptime_secs() -> io::Result<f64> {
    let content = fs::read_to_string("/proc/uptime")?;
    content
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Failed to parse uptime"))
}

fn get_mem_total() -> Option<u64> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse().ok()?;
                return Some(kb * 1024); // Convert KB to bytes
            }
        }
    }
    None
}

fn get_cmdline(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let content = fs::read_to_string(&cmdline_path).ok()?;

    // cmdline is null-separated, get first argument (program name)
    let first_arg = content.split('\0').next()?;
    if first_arg.is_empty() {
        return None;
    }

    // Extract just the program name (last component of path)
    let program = first_arg.rsplit('/').next().unwrap_or(first_arg);
    Some(program.to_string())
}

pub struct PsConfig {
    pub time_step: f32,
    pub max_procs: usize,
    pub show_kernel: bool,
}

pub fn run(config: PsConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step, 0.5);
    let mut monitor = PsMonitor::new(config.show_kernel);
    const HELP: HelpSpec = HelpSpec::animated("PROCESS LIST", &[HelpEntry::new("m", "Cycle sort")]);
    let mut show_help = false;

    monitor.update()?;
    std::thread::sleep(std::time::Duration::from_millis(100));

    loop {
        if let Ok(Some((code, mods))) = term.check_key() {
            if code == KeyCode::Char('?') {
                show_help = !show_help;
            } else if code == KeyCode::Char('m') {
                monitor.toggle_sort();
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

        let (w, h) = term.size();
        monitor.render(
            &mut term,
            w as usize,
            h as usize,
            &state.colors,
            config.max_procs,
        );

        if show_help {
            let (w, h) = term.size();
            render_help_spec(&mut term, w, h, &HELP);
        }

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
