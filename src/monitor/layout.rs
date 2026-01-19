use crate::colors::{ColorState, scheme_color};
use crate::terminal::Terminal;
use crossterm::style::Color;

/// A bounding box for layout calculations
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    /// Inner content area (excluding borders)
    pub fn inner_x(&self) -> i32 { self.x + 1 }
    pub fn inner_y(&self) -> i32 { self.y + 1 }
    pub fn inner_width(&self) -> u16 { self.width.saturating_sub(2) }
    pub fn inner_height(&self) -> u16 { self.height.saturating_sub(2) }
}

/// Draw a btop-style meter with color scheme support
pub fn draw_meter_btop_scheme(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    colors: &ColorState,
) {
    if width == 0 { return; }

    const METER_CHAR: char = '■';
    let filled = ((percent / 100.0) * width as f32) as usize;

    for i in 0..width {
        if i < filled {
            let pos_pct = (i as f32 / width as f32) * 100.0;
            let grad = cpu_gradient_color_scheme(pos_pct.min(percent), colors);
            term.set(x + i as i32, y, METER_CHAR, Some(grad), false);
        } else {
            term.set(x + i as i32, y, METER_CHAR, Some(muted_color_scheme(colors)), false);
        }
    }
}

/// Draw per-core meters with temps and color scheme support
#[allow(clippy::too_many_arguments)]
pub fn draw_core_graphs_scheme(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    usage: &[f32],
    temps: &[Option<u32>],
    colors: &ColorState,
) {
    if usage.is_empty() || height == 0 { return; }

    let cores = usage.len();
    let has_temps = !temps.is_empty();

    let cols = 2;
    let col_width = width.saturating_sub(1) / cols;
    let rows_per_col = cores.div_ceil(cols);
    let actual_rows = rows_per_col.min(height);

    let label_w = 4;
    let pct_w = 5;
    let temp_section_w = if has_temps { 1 + 5 + 6 } else { 0 };
    let fixed_w = label_w + pct_w + temp_section_w;

    let usage_meter_w = col_width.saturating_sub(fixed_w).max(5);
    let temp_meter_w = if has_temps { 5 } else { 0 };

    for row in 0..actual_rows {
        for col in 0..cols {
            let idx = col * rows_per_col + row;
            if idx >= cores { continue; }

            if col > 0 {
                term.set(x + col_width as i32, y + row as i32, '│', Some(muted_color_scheme(colors)), false);
            }

            let cx = x + (col * (col_width + 1)) as i32;
            let cy = y + row as i32;
            let pct = usage[idx];
            let mut pos = cx;

            let label = format!("{:<4}", format!("C{}", idx));
            term.set_str(pos, cy, &label, Some(text_color_scheme(colors)), false);
            pos += label_w as i32;

            if usage_meter_w > 0 {
                draw_meter_btop_scheme(term, pos, cy, usage_meter_w, pct, colors);
                pos += usage_meter_w as i32;
            }

            let pct_str = format!("{:4.0}%", pct);
            term.set_str(pos, cy, &pct_str, Some(cpu_gradient_color_scheme(pct, colors)), false);
            pos += 5;

            if temp_meter_w > 0 {
                pos += 1;
                if let Some(Some(temp)) = temps.get(idx) {
                    let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                    draw_meter_btop_scheme(term, pos, cy, temp_meter_w, temp_pct, colors);
                }
                pos += temp_meter_w as i32;
            }

            if let Some(Some(temp)) = temps.get(idx) {
                let temp_str = format!("  {:2}°C", temp);
                let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                term.set_str(pos, cy, &temp_str, Some(temp_gradient_color_scheme(temp_pct, colors)), false);
            }
        }
    }
}

/// Format bytes with adaptive precision
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}TiB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KiB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Format a rate value (bytes/sec)
pub fn format_rate(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.1}GiB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1}MiB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1}KiB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0}B/s", bytes_per_sec)
    }
}

/// CPU mini graph gradient (btop TTY: bright green -> bright red based on VALUE)
/// Maps percentage 0-100 to ANSI bright green (10) → bright yellow (11) → bright red (9)
pub fn cpu_gradient_color(percent: f32) -> Color {
    if percent >= 80.0 {
        Color::AnsiValue(9)   // Bright red (ANSI 91)
    } else if percent >= 50.0 {
        Color::AnsiValue(11)  // Bright yellow (ANSI 93)
    } else {
        Color::AnsiValue(10)  // Bright green (ANSI 92)
    }
}

/// Temperature mini graph gradient (btop TTY: bright blue -> bright magenta based on VALUE)
/// Maps temperature percentage to ANSI bright blue (12) → bright magenta (13)
pub fn temp_gradient_color(percent: f32) -> Color {
    if percent >= 70.0 {
        Color::AnsiValue(13)  // Bright magenta (ANSI 95)
    } else if percent >= 40.0 {
        Color::AnsiValue(14)  // Bright cyan (ANSI 96) - mid point
    } else {
        Color::AnsiValue(12)  // Bright blue (ANSI 94)
    }
}

// ============ Scheme-aware color functions ============

/// Get CPU gradient color with scheme support
pub fn cpu_gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    if colors.is_mono() {
        cpu_gradient_color(percent)
    } else {
        let intensity = if percent >= 80.0 { 3 } else if percent >= 50.0 { 2 } else { 1 };
        scheme_color(colors.scheme, intensity, percent >= 80.0).0
    }
}

/// Get temperature gradient color with scheme support
pub fn temp_gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    if colors.is_mono() {
        temp_gradient_color(percent)
    } else {
        let intensity = if percent >= 70.0 { 3 } else if percent >= 40.0 { 2 } else { 1 };
        scheme_color(colors.scheme, intensity, percent >= 70.0).0
    }
}

/// Get text color with scheme support
pub fn text_color_scheme(colors: &ColorState) -> Color {
    if colors.is_mono() {
        Color::White
    } else {
        scheme_color(colors.scheme, 2, false).0
    }
}

/// Get muted color with scheme support
pub fn muted_color_scheme(colors: &ColorState) -> Color {
    if colors.is_mono() {
        Color::DarkGrey
    } else {
        scheme_color(colors.scheme, 0, false).0
    }
}

/// Get header color with scheme support
pub fn header_color_scheme(colors: &ColorState) -> Color {
    if colors.is_mono() {
        Color::Cyan
    } else {
        scheme_color(colors.scheme, 3, true).0
    }
}


