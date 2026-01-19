use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::{MonitorConfig, MonitorState};
use crate::monitor::layout::{
    Rect, draw_meter_btop_scheme, cpu_gradient_color_scheme, format_bytes,
    muted_color_scheme, header_color_scheme, temp_gradient_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::fs;
use std::io;
use std::process::Command;

struct GpuInfo {
    name: String,
    utilization: f32,
    memory_used: u64,
    memory_total: u64,
    temperature: Option<u32>,
    power_draw: Option<f32>,
    power_limit: Option<f32>,
    fan_speed: Option<u32>,
    fan_max: Option<u32>,
}

impl GpuInfo {
    fn memory_percent(&self) -> f32 {
        if self.memory_total > 0 {
            (self.memory_used as f32 / self.memory_total as f32) * 100.0
        } else {
            0.0
        }
    }

    fn fan_percent(&self) -> Option<f32> {
        match (self.fan_speed, self.fan_max) {
            (Some(speed), Some(max)) if max > 0 => Some((speed as f32 / max as f32) * 100.0),
            (Some(speed), None) => Some(speed as f32), // Assume it's already a percentage
            _ => None,
        }
    }
}

#[derive(PartialEq)]
enum GpuBackend {
    Nvidia,
    Amd,
    None,
}

pub struct GpuMonitor {
    gpus: Vec<GpuInfo>,
    pub history_util: Vec<Vec<f32>>,
    backend: GpuBackend,
    amd_card_path: Option<String>,
    error_msg: Option<String>,
}

impl GpuMonitor {
    pub fn new() -> Self {
        // Check for NVIDIA first
        let has_nvidia = Command::new("nvidia-smi")
            .arg("--query")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if has_nvidia {
            return Self {
                gpus: Vec::new(),
                history_util: Vec::new(),
                backend: GpuBackend::Nvidia,
                amd_card_path: None,
                error_msg: None,
            };
        }

        // Check for AMD GPU
        let amd_card_path = Self::find_amd_gpu();
        if amd_card_path.is_some() {
            return Self {
                gpus: Vec::new(),
                history_util: Vec::new(),
                backend: GpuBackend::Amd,
                amd_card_path,
                error_msg: None,
            };
        }

        Self {
            gpus: Vec::new(),
            history_util: Vec::new(),
            backend: GpuBackend::None,
            amd_card_path: None,
            error_msg: Some("No GPU detected".to_string()),
        }
    }

    fn find_amd_gpu() -> Option<String> {
        // Look for AMD GPU in /sys/class/drm/card*/device/
        for entry in fs::read_dir("/sys/class/drm").ok()? {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("card") && !name.contains('-') {
                let path = format!("/sys/class/drm/{}/device", name);
                // Check if it has gpu_busy_percent (AMD-specific)
                if fs::metadata(format!("{}/gpu_busy_percent", path)).is_ok() {
                    return Some(path);
                }
            }
        }
        None
    }

    fn read_sysfs(path: &str) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    fn read_sysfs_u64(path: &str) -> Option<u64> {
        Self::read_sysfs(path)?.parse().ok()
    }

    fn read_sysfs_u32(path: &str) -> Option<u32> {
        Self::read_sysfs(path)?.parse().ok()
    }

    fn get_amd_gpu_name() -> String {
        // Try lspci for a nice name
        if let Ok(output) = Command::new("lspci").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("VGA") && line.contains("AMD") {
                    // Extract the model name
                    if let Some(start) = line.find('[') {
                        if let Some(end) = line.rfind(']') {
                            let name = &line[start + 1..end];
                            // Remove "AMD/ATI" prefix if present
                            return name.replace("AMD/ATI] ", "").replace("AMD/ATI ", "");
                        }
                    }
                }
            }
        }
        "AMD GPU".to_string()
    }

    pub fn update(&mut self) -> io::Result<()> {
        match self.backend {
            GpuBackend::Nvidia => self.update_nvidia(),
            GpuBackend::Amd => self.update_amd(),
            GpuBackend::None => Ok(()),
        }
    }

    fn update_nvidia(&mut self) -> io::Result<()> {
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,power.limit,fan.speed")
            .arg("--format=csv,noheader,nounits")
            .output();

        match output {
            Ok(out) => {
                if !out.status.success() {
                    self.error_msg = Some("nvidia-smi failed".to_string());
                    return Ok(());
                }

                let stdout = String::from_utf8_lossy(&out.stdout);
                self.gpus.clear();

                for (i, line) in stdout.lines().enumerate() {
                    let parts: Vec<&str> = line.split(", ").collect();
                    if parts.len() >= 4 {
                        let gpu = GpuInfo {
                            name: parts[0].trim().to_string(),
                            utilization: parts[1].trim().parse().unwrap_or(0.0),
                            memory_used: (parts[2].trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0) as u64,
                            memory_total: (parts[3].trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0) as u64,
                            temperature: parts.get(4).and_then(|s| s.trim().parse().ok()),
                            power_draw: parts.get(5).and_then(|s| s.trim().parse().ok()),
                            power_limit: parts.get(6).and_then(|s| s.trim().parse().ok()),
                            fan_speed: parts.get(7).and_then(|s| s.trim().parse().ok()),
                            fan_max: Some(100), // NVIDIA reports percentage directly
                        };

                        if i >= self.history_util.len() {
                            self.history_util.push(Vec::new());
                        }
                        self.history_util[i].push(gpu.utilization);
                        if self.history_util[i].len() > 500 {
                            self.history_util[i].remove(0);
                        }

                        self.gpus.push(gpu);
                    }
                }

                self.error_msg = None;
            }
            Err(e) => {
                self.error_msg = Some(format!("Error: {}", e));
            }
        }

        Ok(())
    }

    fn update_amd(&mut self) -> io::Result<()> {
        let Some(ref card_path) = self.amd_card_path else {
            return Ok(());
        };

        // Find hwmon path
        let hwmon_path = fs::read_dir(format!("{}/hwmon", card_path))
            .ok()
            .and_then(|mut entries| entries.next())
            .and_then(|e| e.ok())
            .map(|e| e.path().to_string_lossy().to_string());

        let utilization = Self::read_sysfs_u32(&format!("{}/gpu_busy_percent", card_path))
            .unwrap_or(0) as f32;

        let memory_used = Self::read_sysfs_u64(&format!("{}/mem_info_vram_used", card_path))
            .unwrap_or(0);
        let memory_total = Self::read_sysfs_u64(&format!("{}/mem_info_vram_total", card_path))
            .unwrap_or(0);

        let (temperature, power_draw, power_limit, fan_speed, fan_max) = if let Some(ref hwmon) = hwmon_path {
            let temp = Self::read_sysfs_u32(&format!("{}/temp1_input", hwmon))
                .map(|t| t / 1000); // millicelsius to celsius
            let power = Self::read_sysfs_u64(&format!("{}/power1_average", hwmon))
                .map(|p| p as f32 / 1_000_000.0); // microwatts to watts
            let power_cap = Self::read_sysfs_u64(&format!("{}/power1_cap", hwmon))
                .map(|p| p as f32 / 1_000_000.0);
            let fan = Self::read_sysfs_u32(&format!("{}/fan1_input", hwmon));
            let fan_m = Self::read_sysfs_u32(&format!("{}/fan1_max", hwmon));
            (temp, power, power_cap, fan, fan_m)
        } else {
            (None, None, None, None, None)
        };

        let gpu = GpuInfo {
            name: Self::get_amd_gpu_name(),
            utilization,
            memory_used,
            memory_total,
            temperature,
            power_draw,
            power_limit,
            fan_speed,
            fan_max,
        };

        if self.history_util.is_empty() {
            self.history_util.push(Vec::new());
        }
        self.history_util[0].push(gpu.utilization);
        if self.history_util[0].len() > 500 {
            self.history_util[0].remove(0);
        }

        self.gpus.clear();
        self.gpus.push(gpu);
        self.error_msg = None;

        Ok(())
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

    fn render_at(&self, term: &mut Terminal, x: i32, y: i32, w: usize, h: usize, colors: &ColorState) {
        if h < 4 || w < 30 { return; }

        if let Some(ref err) = self.error_msg {
            let cy = y + (h as i32 / 2);
            term.set_str(x, cy, err, Some(Color::Red), false);
            return;
        }

        if self.gpus.is_empty() {
            let cy = y + (h as i32 / 2);
            term.set_str(x, cy, "No GPU detected", Some(Color::Yellow), false);
            return;
        }

        // Calculate panel height per GPU
        // Each GPU: Name(1) + GPU util(1) + VRAM(1) + Temp(1) + Power(1) + Fan(1) + blank(1) = 7 lines
        let lines_per_gpu = 7;
        let total_height = self.gpus.len() * lines_per_gpu - 1; // -1 for no trailing blank

        // Vertically center
        let start_y = y + ((h as i32 - total_height as i32) / 2).max(0);

        let mut cy = start_y;

        for (i, gpu) in self.gpus.iter().enumerate() {
            // GPU name with temp aligned right
            let temp_str = gpu.temperature.map(|t| format!("{:4}°C", t));
            let temp_len = temp_str.as_ref().map(|s| s.len()).unwrap_or(0);
            let max_name_len = w.saturating_sub(temp_len + 2); // +2 for spacing

            let name_display = if gpu.name.chars().count() > max_name_len {
                let truncated: String = gpu.name.chars().take(max_name_len.saturating_sub(1)).collect();
                format!("{}…", truncated)
            } else {
                gpu.name.clone()
            };

            term.set_str(x, cy, &name_display, Some(header_color_scheme(colors)), true);
            if let Some(temp) = gpu.temperature {
                let temp_pct = ((temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                let temp_color = temp_gradient_color_scheme(temp_pct, colors);
                let ts = format!("{:4}°C", temp);
                term.set_str(x + w as i32 - ts.len() as i32, cy, &ts, Some(temp_color), false);
            }
            cy += 1;

            // GPU utilization
            self.draw_gpu_row(term, x, cy, w, "GPU", gpu.utilization, None, colors, true);
            cy += 1;

            // VRAM
            let mem_pct = gpu.memory_percent();
            let mem_str = format!("{}/{}", format_bytes(gpu.memory_used), format_bytes(gpu.memory_total));
            self.draw_gpu_row(term, x, cy, w, "VRAM", mem_pct, Some(&mem_str), colors, false);
            cy += 1;

            // Power
            if let (Some(draw), Some(limit)) = (gpu.power_draw, gpu.power_limit) {
                let power_pct = (draw / limit * 100.0).min(100.0);
                let power_str = format!("{:.0}W/{:.0}W", draw, limit);
                self.draw_gpu_row(term, x, cy, w, "Power", power_pct, Some(&power_str), colors, false);
            } else {
                term.set_str(x, cy, "Power   N/A", Some(muted_color_scheme(colors)), false);
            }
            cy += 1;

            // Fan
            if let Some(fan_pct) = gpu.fan_percent() {
                let fan_str = gpu.fan_speed.map(|rpm| format!("{}RPM", rpm));
                self.draw_gpu_row(term, x, cy, w, "Fan", fan_pct, fan_str.as_deref(), colors, false);
            } else {
                term.set_str(x, cy, "Fan     N/A", Some(muted_color_scheme(colors)), false);
            }
            cy += 1;

            // Blank line between GPUs (except last)
            if i < self.gpus.len() - 1 {
                cy += 1;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_gpu_row(&self, term: &mut Terminal, x: i32, y: i32, width: usize, label: &str, percent: f32, value_str: Option<&str>, colors: &ColorState, is_util: bool) {
        // Layout: Label(8) + Meter(dynamic) + Pct(6) + Value(16) - always reserve value space
        let label_w = 8;
        let pct_w = 6;
        let value_w = 16; // Always reserve space for alignment
        let meter_w = width.saturating_sub(label_w + pct_w + value_w);

        let mut pos = x;

        // Label
        let label_str = format!("{:<8}", label);
        term.set_str(pos, y, &label_str, Some(muted_color_scheme(colors)), false);
        pos += label_w as i32;

        // Color based on scheme
        let color = if is_util {
            cpu_gradient_color_scheme(percent, colors)
        } else if colors.is_mono() {
            Color::AnsiValue(12)  // Blue for non-util items in mono
        } else {
            cpu_gradient_color_scheme(50.0, colors)  // Mid-intensity for non-util
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

        // Value (right-aligned in fixed space)
        if let Some(val) = value_str {
            let val_pad = value_w.saturating_sub(val.len());
            term.set_str(pos + val_pad as i32, y, val, Some(muted_color_scheme(colors)), false);
        }
    }
}

pub fn run(config: MonitorConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(0.5));
    let mut monitor = GpuMonitor::new();

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

        let (w, h) = term.size();
        monitor.render_fullscreen(&mut term, w as usize, h as usize, &state.colors);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
