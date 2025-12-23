//! Docker container monitor - shows container resource usage

use crate::colors::ColorState;
use crate::terminal::Terminal;
use crate::monitor::MonitorState;
use crate::monitor::layout::{
    cpu_gradient_color_scheme, text_color_scheme, muted_color_scheme,
};
use crossterm::style::Color;
use crossterm::terminal::size;
use std::io;
use std::process::Command;

#[derive(Clone)]
struct ContainerInfo {
    name: String,
    cpu_pct: f32,
    mem_usage: String,
    mem_pct: f32,
    net_io: String,
}

pub struct DockerMonitor {
    containers: Vec<ContainerInfo>,
    docker_available: bool,
    error_msg: Option<String>,
}

impl DockerMonitor {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            docker_available: true,
            error_msg: None,
        }
    }

    pub fn update(&mut self) -> io::Result<()> {
        // Run docker stats with custom format
        let output = Command::new("docker")
            .args([
                "stats",
                "--no-stream",
                "--format",
                "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}\t{{.NetIO}}",
            ])
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if stderr.contains("Cannot connect") || stderr.contains("permission denied") {
                        self.docker_available = false;
                        self.error_msg = Some("Docker daemon not accessible".to_string());
                    } else {
                        self.error_msg = Some(stderr.trim().to_string());
                    }
                    self.containers.clear();
                    return Ok(());
                }

                self.docker_available = true;
                self.error_msg = None;

                let stdout = String::from_utf8_lossy(&result.stdout);
                self.containers = stdout
                    .lines()
                    .filter(|line| !line.is_empty())
                    .filter_map(|line| parse_container_line(line))
                    .collect();
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    self.docker_available = false;
                    self.error_msg = Some("Docker not installed".to_string());
                } else {
                    self.error_msg = Some(format!("Error: {}", e));
                }
                self.containers.clear();
            }
        }

        Ok(())
    }

    pub fn render(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        if h < 3 || w < 40 {
            return;
        }

        let header_y = 0;
        let mut y = 1;

        // Title
        let title = "Docker Containers";
        let count_str = format!("[{}]", self.containers.len());
        term.set_str(0, header_y, title, Some(text_color_scheme(colors)), true);
        term.set_str((w - count_str.len()) as i32, header_y, &count_str, Some(muted_color_scheme(colors)), false);

        // Error state
        if !self.docker_available || self.error_msg.is_some() {
            let msg = self.error_msg.as_deref().unwrap_or("Docker unavailable");
            term.set_str(0, y, msg, Some(Color::Red), false);
            return;
        }

        // No containers
        if self.containers.is_empty() {
            term.set_str(0, y, "No running containers", Some(muted_color_scheme(colors)), false);
            return;
        }

        y += 1;

        // Column header
        let header = format!(
            "{:<20} {:>8} {:>16} {:>8} {:>16}",
            "NAME", "CPU%", "MEM USAGE", "MEM%", "NET I/O"
        );
        let header_truncated: String = header.chars().take(w).collect();
        term.set_str(0, y, &header_truncated, Some(text_color_scheme(colors)), false);
        y += 1;

        // Container rows
        for container in &self.containers {
            if y >= h as i32 - 1 {
                break;
            }

            let row = format!(
                "{:<20} {:>8} {:>16} {:>8} {:>16}",
                truncate_str(&container.name, 20),
                format!("{:.1}%", container.cpu_pct),
                container.mem_usage,
                format!("{:.1}%", container.mem_pct),
                container.net_io
            );

            let row_truncated: String = row.chars().take(w).collect();

            // Color based on CPU usage
            let row_color = cpu_gradient_color_scheme(container.cpu_pct.min(100.0), colors);
            term.set_str(0, y, &row_truncated, Some(row_color), false);

            y += 1;
        }

        // Hint line at bottom
        let hint = "q:Quit  Space:Pause  0-9:Speed";
        let hint_y = (h - 1) as i32;
        term.set_str(0, hint_y, hint, Some(muted_color_scheme(colors)), false);
    }
}

fn parse_container_line(line: &str) -> Option<ContainerInfo> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 5 {
        return None;
    }

    let cpu_str = parts[1].trim_end_matches('%');
    let mem_pct_str = parts[3].trim_end_matches('%');

    Some(ContainerInfo {
        name: parts[0].to_string(),
        cpu_pct: cpu_str.parse().unwrap_or(0.0),
        mem_usage: parts[2].to_string(),
        mem_pct: mem_pct_str.parse().unwrap_or(0.0),
        net_io: parts[4].to_string(),
    })
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

pub struct DockerConfig {
    pub time_step: f32,
}

pub fn run(config: DockerConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step.max(0.5));
    let mut monitor = DockerMonitor::new();

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
        monitor.render(&mut term, w as usize, h as usize, &state.colors);

        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
