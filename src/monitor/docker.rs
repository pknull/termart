//! Docker container monitor - shows container resource usage

use crate::colors::ColorState;
use crate::help::{render_help_overlay, HelpEntry, HelpSpec};
use crate::monitor::layout::{cpu_gradient_color_scheme, muted_color_scheme, text_color_scheme};
use crate::monitor::{command_output_with_timeout, truncate_message, MonitorAction, MonitorState};
use crate::terminal::Terminal;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::io;
use std::process::Command;
use std::time::Duration;

const COLLECTOR_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    Cpu,
    Mem,
    Name,
}

impl SortBy {
    fn next(self) -> Self {
        match self {
            SortBy::Cpu => SortBy::Mem,
            SortBy::Mem => SortBy::Name,
            SortBy::Name => SortBy::Cpu,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortBy::Cpu => "CPU%",
            SortBy::Mem => "MEM%",
            SortBy::Name => "NAME",
        }
    }
}

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
    sort_by: SortBy,
    selected_name: Option<String>,
    detail_open: bool,
}

impl DockerMonitor {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            docker_available: true,
            error_msg: None,
            sort_by: SortBy::Cpu,
            selected_name: None,
            detail_open: false,
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_by = self.sort_by.next();
        self.sort_containers();
    }

    fn sort_containers(&mut self) {
        match self.sort_by {
            SortBy::Cpu => self.containers.sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Mem => self.containers.sort_by(|a, b| {
                b.mem_pct
                    .partial_cmp(&a.mem_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Name => self.containers.sort_by(|a, b| a.name.cmp(&b.name)),
        }
    }

    pub fn update(&mut self) -> io::Result<()> {
        // Run docker stats with custom format
        let output = command_output_with_timeout(
            Command::new("docker").args([
                "stats",
                "--no-stream",
                "--format",
                "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}\t{{.NetIO}}",
            ]),
            COLLECTOR_TIMEOUT,
        );

        match output {
            Ok(result) => {
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    let message = if stderr.contains("Cannot connect")
                        || stderr.contains("permission denied")
                    {
                        "Docker daemon not accessible".to_string()
                    } else {
                        let message = stderr
                            .lines()
                            .map(str::trim)
                            .find(|line| !line.is_empty())
                            .unwrap_or("");
                        if message.is_empty() {
                            "docker stats failed".to_string()
                        } else {
                            truncate_message(message, 256)
                        }
                    };
                    if message == "Docker daemon not accessible" {
                        self.docker_available = false;
                    }
                    self.error_msg = Some(message.clone());
                    self.containers.clear();
                    self.reconcile_selection();
                    return Err(io::Error::other(message));
                }

                self.docker_available = true;
                self.error_msg = None;

                let stdout = String::from_utf8_lossy(&result.stdout);
                self.containers = stdout
                    .lines()
                    .filter(|line| !line.is_empty())
                    .filter_map(parse_container_line)
                    .collect();
                self.sort_containers();
                self.reconcile_selection();
            }
            Err(e) => {
                let message = if e.kind() == io::ErrorKind::NotFound {
                    self.docker_available = false;
                    "Docker not installed".to_string()
                } else {
                    format!("Error: {e}")
                };
                self.error_msg = Some(message.clone());
                self.containers.clear();
                self.reconcile_selection();
                return Err(io::Error::new(e.kind(), message));
            }
        }

        Ok(())
    }

    pub fn select_next(&mut self) {
        self.move_selection(1);
    }

    pub fn select_previous(&mut self) {
        self.move_selection(-1);
    }

    pub fn toggle_details(&mut self) {
        if self.selected_container().is_some() {
            self.detail_open = !self.detail_open;
        }
    }

    pub fn close_details(&mut self) {
        self.detail_open = false;
    }

    pub fn selection_label(&self) -> Option<&str> {
        self.selected_container()
            .map(|container| container.name.as_str())
    }

    pub fn detail_text(&self) -> Option<String> {
        let container = self.selected_container()?;
        Some(format!(
            "CONTAINER DETAILS\n───────────────────────\nName      {}\nCPU       {:.1}%\nMemory    {} ({:.1}%)\nNetwork   {}",
            container.name,
            container.cpu_pct,
            container.mem_usage,
            container.mem_pct,
            container.net_io
        ))
    }

    fn move_selection(&mut self, direction: i32) {
        if self.containers.is_empty() {
            self.selected_name = None;
            self.detail_open = false;
            return;
        }

        let current = self
            .selected_name
            .as_deref()
            .and_then(|name| {
                self.containers
                    .iter()
                    .position(|container| container.name == name)
            })
            .unwrap_or(0);
        let next = if direction < 0 {
            current
                .checked_sub(1)
                .unwrap_or(self.containers.len().saturating_sub(1))
        } else {
            (current + 1) % self.containers.len()
        };
        self.selected_name = Some(self.containers[next].name.clone());
    }

    fn reconcile_selection(&mut self) {
        if self.containers.is_empty() {
            self.selected_name = None;
            self.detail_open = false;
        } else if !self.selected_name.as_deref().is_some_and(|name| {
            self.containers
                .iter()
                .any(|container| container.name == name)
        }) {
            self.selected_name = Some(self.containers[0].name.clone());
            self.detail_open = false;
        }
    }

    fn selected_container(&self) -> Option<&ContainerInfo> {
        let name = self.selected_name.as_deref()?;
        self.containers
            .iter()
            .find(|container| container.name == name)
    }

    pub fn render(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        if h < 2 || w < 40 {
            return;
        }

        let header_y = 0;
        let mut y = 1;

        // Title with sort indicator
        let title = "Docker Containers";
        let sort_str = format!("[m]Sort:{}", self.sort_by.label());
        let count_str = format!("[{}]", self.containers.len());
        term.set_str(0, header_y, title, Some(text_color_scheme(colors)), true);
        let sort_x = w.saturating_sub(sort_str.len() + count_str.len() + 2) as i32;
        term.set_str(
            sort_x,
            header_y,
            &sort_str,
            Some(muted_color_scheme(colors)),
            false,
        );
        term.set_str(
            (w - count_str.len()) as i32,
            header_y,
            &count_str,
            Some(muted_color_scheme(colors)),
            false,
        );

        // Error state
        if !self.docker_available || self.error_msg.is_some() {
            let msg = self.error_msg.as_deref().unwrap_or("Docker unavailable");
            term.set_str(0, y, msg, Some(Color::Red), false);
            return;
        }

        // No containers
        if self.containers.is_empty() {
            term.set_str(
                0,
                y,
                "No running containers",
                Some(muted_color_scheme(colors)),
                false,
            );
            return;
        }

        // Column header
        let header = format!(
            "{:<20} {:>8} {:>16} {:>8} {:>16}",
            "NAME", "CPU%", "MEM USAGE", "MEM%", "NET I/O"
        );
        let header_truncated: String = header.chars().take(w).collect();
        term.set_str(
            0,
            y,
            &header_truncated,
            Some(text_color_scheme(colors)),
            false,
        );
        y += 1;

        let available_rows = h.saturating_sub(y as usize);
        let show_count = self.containers.len().min(available_rows);
        let selected_index = self
            .selected_name
            .as_deref()
            .and_then(|name| {
                self.containers
                    .iter()
                    .position(|container| container.name == name)
            })
            .unwrap_or(0);
        let start = selected_index.saturating_add(1).saturating_sub(show_count);

        // Container rows
        for container in self.containers.iter().skip(start).take(show_count) {
            if y >= h as i32 {
                break;
            }

            let row = format!(
                "{}{:<20} {:>8} {:>16} {:>8} {:>16}",
                if self.selected_name.as_deref() == Some(container.name.as_str()) {
                    '>'
                } else {
                    ' '
                },
                truncate_str(&container.name, 20),
                format!("{:.1}%", container.cpu_pct),
                container.mem_usage,
                format!("{:.1}%", container.mem_pct),
                container.net_io
            );

            let row_truncated: String = row.chars().take(w).collect();

            // Color based on CPU usage
            let selected = self.selected_name.as_deref() == Some(container.name.as_str());
            let row_color = if selected {
                text_color_scheme(colors)
            } else {
                cpu_gradient_color_scheme(container.cpu_pct.min(100.0), colors)
            };
            term.set_str(0, y, &row_truncated, Some(row_color), selected);

            y += 1;
        }
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
    // Count/slice by chars, not bytes, to avoid panicking on a multibyte boundary.
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

pub struct DockerConfig {
    pub time_step: f32,
}

pub fn run(config: DockerConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut state = MonitorState::new(config.time_step, 2.0);
    let mut monitor = DockerMonitor::new();
    const HELP: HelpSpec = HelpSpec::monitor(
        "DOCKER STATS",
        &[
            HelpEntry::new("↑/↓ or j/k", "Select container"),
            HelpEntry::new("Enter", "Toggle details"),
            HelpEntry::new("m/s", "Cycle sort"),
        ],
    );

    loop {
        let mut action = MonitorAction::None;
        if let Ok(Some((code, mods))) = term.check_key() {
            use crossterm::event::KeyCode;
            if monitor.detail_open && code == KeyCode::Esc {
                monitor.close_details();
                state.set_feedback("Details closed");
            } else {
                match code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        monitor.select_previous();
                        if let Some(label) = monitor.selection_label() {
                            state.set_feedback(format!("Selected: {label}"));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        monitor.select_next();
                        if let Some(label) = monitor.selection_label() {
                            state.set_feedback(format!("Selected: {label}"));
                        }
                    }
                    KeyCode::Enter => {
                        monitor.toggle_details();
                    }
                    KeyCode::Char('m') | KeyCode::Char('s') => {
                        monitor.cycle_sort();
                        state.set_feedback(format!("Sort: {}", monitor.sort_by.label()));
                    }
                    _ => action = state.handle_key(code, mods),
                }
            }
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
        monitor.render(&mut term, w as usize, h as usize, &state.colors);

        if monitor.detail_open {
            if let Some(details) = monitor.detail_text() {
                render_help_overlay(&mut term, w, h, &details);
            }
        }

        state.render_help(&mut term, w, h, &HELP);

        term.present()?;
        term.sleep(state.poll_delay());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ContainerInfo, DockerMonitor};

    fn container(name: &str, cpu_pct: f32, mem_pct: f32) -> ContainerInfo {
        ContainerInfo {
            name: name.to_string(),
            cpu_pct,
            mem_usage: "100MiB / 1GiB".to_string(),
            mem_pct,
            net_io: "1kB / 2kB".to_string(),
        }
    }

    #[test]
    fn container_selection_wraps_and_survives_sorting() {
        let mut monitor = DockerMonitor::new();
        monitor.containers = vec![
            container("alpha", 90.0, 1.0),
            container("beta", 50.0, 3.0),
            container("gamma", 10.0, 2.0),
        ];
        monitor.reconcile_selection();

        assert_eq!(monitor.selected_name.as_deref(), Some("alpha"));
        monitor.select_previous();
        assert_eq!(monitor.selected_name.as_deref(), Some("gamma"));
        monitor.select_next();
        monitor.select_next();
        assert_eq!(monitor.selected_name.as_deref(), Some("beta"));

        monitor.cycle_sort();
        assert_eq!(monitor.selected_name.as_deref(), Some("beta"));
        assert_eq!(monitor.containers[0].name, "beta");
    }

    #[test]
    fn container_selection_clears_with_the_container_list() {
        let mut monitor = DockerMonitor::new();
        monitor.containers = vec![container("alpha", 1.0, 1.0)];
        monitor.reconcile_selection();
        monitor.toggle_details();
        assert!(monitor.detail_open);

        monitor.containers.clear();
        monitor.reconcile_selection();

        assert!(monitor.selected_name.is_none());
        assert!(!monitor.detail_open);
        assert!(monitor.detail_text().is_none());
    }
}
