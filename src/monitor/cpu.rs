use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::{MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Rect, draw_meter_btop_scheme, draw_core_graphs_scheme,
    cpu_gradient_color_scheme, temp_gradient_color_scheme,
    text_color_scheme, muted_color_scheme,
};
use crossterm::terminal::size;
use std::fs;
use std::io;

fn get_uptime() -> Option<String> {
    let uptime = fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = uptime.split_whitespace().next()?.parse().ok()?;
    let days = (secs / 86400.0) as u64;
    let hours = ((secs % 86400.0) / 3600.0) as u64;
    let mins = ((secs % 3600.0) / 60.0) as u64;
    if days > 0 {
        Some(format!("{}d {:02}:{:02}", days, hours, mins))
    } else {
        Some(format!("{:02}:{:02}", hours, mins))
    }
}

fn get_loadavg() -> Option<(f32, f32, f32)> {
    let content = fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() >= 3 {
        Some((
            parts[0].parse().unwrap_or(0.0),
            parts[1].parse().unwrap_or(0.0),
            parts[2].parse().unwrap_or(0.0),
        ))
    } else {
        None
    }
}

fn get_cpu_model() -> Option<String> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in content.lines() {
        if line.starts_with("model name") {
            return line.split(':').nth(1).map(|s| {
                // Clean up the model name - remove extra spaces and common prefixes
                s.trim()
                    .replace("(R)", "")
                    .replace("(TM)", "")
                    .replace("CPU ", "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        }
    }
    None
}

fn shorten_cpu_model(model: &str, max_len: usize) -> String {
    if model.len() <= max_len {
        return model.to_string();
    }
    // Try to extract just the model number (e.g., "i7-8700K" from "Intel Core i7-8700K @ 3.70GHz")
    let parts: Vec<&str> = model.split_whitespace().collect();
    for part in &parts {
        if part.starts_with("i7") || part.starts_with("i9") || part.starts_with("i5") || part.starts_with("i3")
            || part.starts_with("Ryzen") || part.contains("-") {
            return part.to_string();
        }
    }
    model.chars().take(max_len).collect()
}

fn get_cpu_freq() -> Option<f32> {
    // Try scaling_cur_freq first (more accurate)
    if let Ok(content) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq") {
        if let Ok(khz) = content.trim().parse::<f64>() {
            return Some((khz / 1_000_000.0) as f32);  // Convert kHz to GHz
        }
    }
    // Fallback to /proc/cpuinfo
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if line.starts_with("cpu MHz") {
                if let Some(mhz_str) = line.split(':').nth(1) {
                    if let Ok(mhz) = mhz_str.trim().parse::<f64>() {
                        return Some((mhz / 1000.0) as f32);  // Convert MHz to GHz
                    }
                }
            }
        }
    }
    None
}

fn get_cpu_temp_from_path(thermal_path: Option<&String>) -> Option<u32> {
    if let Some(path) = thermal_path {
        // Check if this is a hwmon path
        if path.contains("/sys/class/hwmon") {
            let temp_path = std::path::Path::new(path).join("temp1_input");
            if let Ok(temp_str) = fs::read_to_string(temp_path) {
                if let Ok(millideg) = temp_str.trim().parse::<i64>() {
                    return Some((millideg / 1000) as u32);
                }
            }
        } else {
            // It's a thermal zone path
            if let Ok(temp_str) = fs::read_to_string(path) {
                if let Ok(millideg) = temp_str.trim().parse::<i64>() {
                    return Some((millideg / 1000) as u32);
                }
            }
        }
    }
    None
}

fn get_core_temps_from_path(thermal_path: Option<&String>, num_logical: usize) -> Vec<Option<u32>> {
    let mut physical_temps = Vec::new();

    // If we have a cached hwmon path, use it directly
    if let Some(path) = thermal_path {
        if path.contains("/sys/class/hwmon") {
            let hwmon_path = std::path::Path::new(path);
            // Read per-core temps (temp2, temp3, etc. are usually cores)
            for i in 2..32 {
                let temp_path = hwmon_path.join(format!("temp{}_input", i));
                if temp_path.exists() {
                    if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                        if let Ok(millideg) = temp_str.trim().parse::<i64>() {
                            physical_temps.push(Some((millideg / 1000) as u32));
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    // Map physical core temps to logical cores (hyperthreading)
    let num_physical = physical_temps.len();
    if num_physical == 0 {
        return vec![None; num_logical];
    }

    let mut temps = Vec::with_capacity(num_logical);
    for i in 0..num_logical {
        let phys_idx = i % num_physical;
        temps.push(physical_temps.get(phys_idx).copied().flatten());
    }
    temps
}

struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuTimes {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }

    fn active(&self) -> u64 {
        self.user + self.nice + self.system + self.irq + self.softirq + self.steal
    }
}

struct CpuState {
    cores: Vec<CpuTimes>,
    total: CpuTimes,
}

pub struct CpuMonitor {
    prev_state: Option<CpuState>,
    pub usage_per_core: Vec<f32>,
    pub usage_total: f32,
    pub iowait_pct: f32,
    // Cached values for expensive operations
    cpu_model: Option<String>,
    cpu_model_short: Option<String>,
    cpu_freq_ghz: Option<f32>,
    freq_update_counter: u32,
    thermal_zone_path: Option<String>, // Cache the working thermal zone
}

impl CpuMonitor {
    pub fn new() -> Self {
        // Initialize with cached values
        let cpu_model = get_cpu_model();
        let cpu_model_short = cpu_model.as_ref().map(|m| {
             // Pre-compute a reasonable default
            shorten_cpu_model(m, 40)
        });
        
        Self {
            prev_state: None,
            usage_per_core: Vec::new(),
            usage_total: 0.0,
            iowait_pct: 0.0,
            cpu_model,
            cpu_model_short,
            cpu_freq_ghz: get_cpu_freq(),
            freq_update_counter: 0,
            thermal_zone_path: Self::discover_thermal_zone(),
        }
    }
    
    // Discover the thermal zone once at startup
    fn discover_thermal_zone() -> Option<String> {
        // Try hwmon coretemp first (Intel)
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(name) = fs::read_to_string(path.join("name")) {
                    let name = name.trim();
                    if name == "coretemp" || name == "k10temp" || name == "zenpower" {
                        // Verify temp1_input exists
                        if path.join("temp1_input").exists() {
                            return Some(path.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
        
        // Fallback to thermal zones - find the first one that works
        for i in 0..10 {
            let zone_path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
            if fs::metadata(&zone_path).is_ok() {
                return Some(zone_path);
            }
        }
        None
    }

    fn read_state() -> io::Result<CpuState> {
        let content = fs::read_to_string("/proc/stat")?;
        let mut cores = Vec::new();
        let mut total = None;

        for line in content.lines() {
            if line.starts_with("cpu") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 8 {
                    let times = CpuTimes {
                        user: parts[1].parse().unwrap_or(0),
                        nice: parts[2].parse().unwrap_or(0),
                        system: parts[3].parse().unwrap_or(0),
                        idle: parts[4].parse().unwrap_or(0),
                        iowait: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
                        irq: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
                        softirq: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0),
                        steal: parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0),
                    };

                    if parts[0] == "cpu" {
                        total = Some(times);
                    } else {
                        cores.push(times);
                    }
                }
            }
        }

        Ok(CpuState {
            cores,
            total: total.unwrap_or(CpuTimes {
                user: 0, nice: 0, system: 0, idle: 0,
                iowait: 0, irq: 0, softirq: 0, steal: 0,
            }),
        })
    }

    pub fn update(&mut self) -> io::Result<()> {
        let current = Self::read_state()?;

        if let Some(ref prev) = self.prev_state {
            let total_diff = current.total.total().saturating_sub(prev.total.total());
            let active_diff = current.total.active().saturating_sub(prev.total.active());
            let iowait_diff = current.total.iowait.saturating_sub(prev.total.iowait);

            self.usage_total = if total_diff > 0 {
                (active_diff as f32 / total_diff as f32) * 100.0
            } else {
                0.0
            };

            self.iowait_pct = if total_diff > 0 {
                (iowait_diff as f32 / total_diff as f32) * 100.0
            } else {
                0.0
            };

            self.usage_per_core.clear();
            for (i, core) in current.cores.iter().enumerate() {
                if i < prev.cores.len() {
                    let total_diff = core.total().saturating_sub(prev.cores[i].total());
                    let active_diff = core.active().saturating_sub(prev.cores[i].active());
                    let usage = if total_diff > 0 {
                        (active_diff as f32 / total_diff as f32) * 100.0
                    } else {
                        0.0
                    };
                    self.usage_per_core.push(usage);
                }
            }
        }

        // Update CPU frequency every 10 updates (reduces file I/O)
        self.freq_update_counter += 1;
        if self.freq_update_counter >= 10 {
            self.cpu_freq_ghz = get_cpu_freq();
            self.freq_update_counter = 0;
        }

        self.prev_state = Some(current);
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

        if h < 3 || w < 20 { return; }

        // Full-width layout, no graph
        let info_w = w;
        let info_x = x;

        // Calculate info panel height: header(1) + CPU meter(1) + cores + load(1)
        let num_cores = self.usage_per_core.len();
        let cores_rows = num_cores.div_ceil(2);  // 2 columns
        let info_height = 2 + cores_rows + 1;  // header + CPU + cores + load

        // Position info panel vertically centered
        let info_y = y + ((h as i32 - info_height as i32) / 2).max(0);

        let mut cy = info_y;

        // Use full width
        let core_section_w = info_w;

        // CPU model and frequency - use cached values
        let freq_str = self.cpu_freq_ghz
            .map(|f| format!("{:.0} MHz", f * 1000.0))
            .unwrap_or_default();

        let max_model_len = core_section_w.saturating_sub(freq_str.len() + 2);
        
        // Recompute short model if width changed significantly
        let model_short = if let Some(ref model) = self.cpu_model {
            if self.cpu_model_short.as_ref().is_none_or(|s| s.len() > max_model_len) {
                shorten_cpu_model(model, max_model_len)
            } else {
                self.cpu_model_short.clone().unwrap_or_else(|| model.clone())
            }
        } else {
            "Unknown CPU".to_string()
        };

        term.set_str(info_x, cy, &model_short, Some(text_color_scheme(colors)), true);
        if !freq_str.is_empty() {
            term.set_str(info_x + core_section_w as i32 - freq_str.len() as i32, cy, &freq_str, Some(cpu_gradient_color_scheme(30.0, colors)), false);
        }
        cy += 1;

        // Total CPU meter - align with core layout below
        // Layout: label(4) + meter(dynamic) + pct(5) + space(1) + temp_meter(5) + temp(6)
        let pkg_temp = get_cpu_temp_from_path(self.thermal_zone_path.as_ref());
        let col_width = (info_w - 1) / 2;  // Match core column width
        let label_w = 4;
        let pct_w = 5;
        let temp_section_w = 1 + 5 + 6;  // space + temp_meter + temp_value
        let meter_w = col_width.saturating_sub(label_w + pct_w + temp_section_w).max(5);

        let mut pos = info_x;

        // "CPU " label (4 chars, same as core labels)
        term.set_str(pos, cy, "CPU ", Some(text_color_scheme(colors)), false);
        pos += label_w as i32;

        // Meter bar (dynamic width to match core meters)
        draw_meter_btop_scheme(term, pos, cy, meter_w, self.usage_total, colors);
        pos += meter_w as i32;

        // Percentage (5 chars right-aligned to match core pct position)
        let pct_str = format!("{:4.0}%", self.usage_total);
        term.set_str(pos, cy, &pct_str, Some(cpu_gradient_color_scheme(self.usage_total, colors)), false);
        pos += pct_w as i32 + 1;  // 5 chars + 1 space

        // Temperature meter (5 chars to match core temp meter width)
        let temp_meter_w = 5;
        if let Some(temp) = pkg_temp {
            let temp_pct = ((temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
            draw_meter_btop_scheme(term, pos, cy, temp_meter_w, temp_pct, colors);
        }
        pos += temp_meter_w as i32;

        // Temperature (6 chars: "  XX°C" to match core temp)
        if let Some(temp) = pkg_temp {
            let temp_str = format!("  {:2}°C", temp);
            let temp_pct = ((temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
            term.set_str(pos, cy, &temp_str, Some(temp_gradient_color_scheme(temp_pct, colors)), false);
        }
        cy += 1;

        // Per-core meters with temps (linear meter style)
        if !self.usage_per_core.is_empty() {
            let core_temps = get_core_temps_from_path(self.thermal_zone_path.as_ref(), self.usage_per_core.len());
            draw_core_graphs_scheme(
                term, info_x, cy, info_w, cores_rows,
                &self.usage_per_core,
                &core_temps,
                colors,
            );
            cy += cores_rows as i32;
        }

        // Uptime (left), IO Wait (after uptime), Load average (right) on same line
        let uptime_str = get_uptime().unwrap_or_else(|| "??:??".to_string());
        let up_str = format!("up {}", uptime_str);
        term.set_str(info_x, cy, &up_str, Some(muted_color_scheme(colors)), false);

        // IO Wait after uptime - yellow/orange color since it indicates blocked I/O
        let iowait_str = format!("  IO Wait: {:4.1}%", self.iowait_pct);
        let iowait_color = if self.iowait_pct > 20.0 {
            crossterm::style::Color::Red
        } else if self.iowait_pct > 5.0 {
            crossterm::style::Color::Yellow
        } else {
            muted_color_scheme(colors)
        };
        let iowait_x = info_x + up_str.len() as i32;
        term.set_str(iowait_x, cy, &iowait_str, Some(iowait_color), false);

        let load = get_loadavg().unwrap_or((0.0, 0.0, 0.0));
        let lav_str = format!("Load: {:.2}  {:.2}  {:.2}", load.0, load.1, load.2);
        term.set_str(info_x + core_section_w as i32 - lav_str.len() as i32, cy, &lav_str, Some(muted_color_scheme(colors)), false);
    }
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(0.5));
    let mut monitor = CpuMonitor::new();

    monitor.update()?;
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
            monitor.update()?;
        }

        term.clear();

        // Render without border
        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
