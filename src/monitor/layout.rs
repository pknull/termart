use crate::colors::{ColorState, scheme_color};
use crate::terminal::Terminal;
use crossterm::style::Color;

// Box drawing characters (rounded)
pub const BOX_TL: char = '╭';  // top-left
pub const BOX_TR: char = '╮';  // top-right
pub const BOX_BL: char = '╰';  // bottom-left
pub const BOX_BR: char = '╯';  // bottom-right
pub const BOX_H: char = '─';   // horizontal
pub const BOX_V: char = '│';   // vertical
pub const BOX_TITLE_L: char = '┤';  // title left bracket
pub const BOX_TITLE_R: char = '├';  // title right bracket

// Partial block characters for smooth meters (1/8 increments)
pub const BLOCKS: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];



/// A bordered box with title (btop-style)
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    pub title: String,
    pub title_color: Color,
    pub border_color: Color,
}

impl Box {
    pub fn new(x: i32, y: i32, width: u16, height: u16, title: &str) -> Self {
        Self {
            x,
            y,
            width,
            height,
            title: title.to_string(),
            title_color: Color::White,
            border_color: Color::DarkGrey,
        }
    }

    /// Inner content area (excluding borders)
    pub fn inner_x(&self) -> i32 { self.x + 1 }
    pub fn inner_y(&self) -> i32 { self.y + 1 }
    pub fn inner_width(&self) -> u16 { self.width.saturating_sub(2) }
    pub fn inner_height(&self) -> u16 { self.height.saturating_sub(2) }

    /// Draw the box border and title
    pub fn draw(&self, term: &mut Terminal) {
        let w = self.width as i32;
        let h = self.height as i32;
        let bc = Some(self.border_color);

        // Top border with title
        term.set(self.x, self.y, BOX_TL, bc, false);

        // Calculate title position (centered)
        let title_start = if !self.title.is_empty() {
            let title_w = self.title.len() + 4; // "┤ title ├"
            let padding = ((w - 2) as usize).saturating_sub(title_w) / 2;

            // Draw left padding
            for i in 1..=padding as i32 {
                term.set(self.x + i, self.y, BOX_H, bc, false);
            }

            // Draw title brackets and title
            let tx = self.x + 1 + padding as i32;
            term.set(tx, self.y, BOX_TITLE_L, bc, false);
            term.set(tx + 1, self.y, ' ', None, false);
            term.set_str(tx + 2, self.y, &self.title, Some(self.title_color), true);
            term.set(tx + 2 + self.title.len() as i32, self.y, ' ', None, false);
            term.set(tx + 3 + self.title.len() as i32, self.y, BOX_TITLE_R, bc, false);

            // Return where right padding starts
            tx + 4 + self.title.len() as i32
        } else {
            self.x + 1
        };

        // Draw right padding of top border
        for i in title_start..(self.x + w - 1) {
            term.set(i, self.y, BOX_H, bc, false);
        }
        term.set(self.x + w - 1, self.y, BOX_TR, bc, false);

        // Side borders
        for i in 1..(h - 1) {
            term.set(self.x, self.y + i, BOX_V, bc, false);
            term.set(self.x + w - 1, self.y + i, BOX_V, bc, false);
        }

        // Bottom border
        term.set(self.x, self.y + h - 1, BOX_BL, bc, false);
        for i in 1..(w - 1) {
            term.set(self.x + i, self.y + h - 1, BOX_H, bc, false);
        }
        term.set(self.x + w - 1, self.y + h - 1, BOX_BR, bc, false);
    }
}

/// Draw a smooth meter using partial block characters
#[allow(dead_code)]
pub fn draw_meter_smooth(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    color: Color,
) {
    if width == 0 { return; }

    let fill = (percent / 100.0) * width as f32;
    let full_blocks = fill as usize;
    let partial = ((fill - full_blocks as f32) * 8.0) as usize;

    for i in 0..width {
        let (ch, c) = if i < full_blocks {
            ('█', Some(color))
        } else if i == full_blocks && partial > 0 {
            (BLOCKS[partial], Some(color))
        } else {
            ('░', Some(Color::DarkGrey))
        };
        term.set(x + i as i32, y, ch, c, false);
    }
}

/// Draw a btop-style meter using solid blocks with gradient color
/// btop uses '■' for both filled and empty, differentiated by color
/// Uses green→yellow→red gradient like btop
pub fn draw_meter_btop(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    _color: Color,  // unused - gradient calculated internally
) {
    if width == 0 { return; }

    const METER_CHAR: char = '■';  // btop's meter character
    let filled = ((percent / 100.0) * width as f32) as usize;

    for i in 0..width {
        if i < filled {
            // Filled portion - use cpu gradient (green→yellow→red) based on position
            let pos_pct = (i as f32 / width as f32) * 100.0;
            let grad = cpu_gradient_color(pos_pct.min(percent));
            term.set(x + i as i32, y, METER_CHAR, Some(grad), false);
        } else {
            // Empty portion - dark background
            term.set(x + i as i32, y, METER_CHAR, Some(Color::DarkGrey), false);
        }
    }
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

/// Draw per-core meters with temps (linear meter style)
/// Format: C0  ■■■■■■■■■■   0% ■■■■■  29°C│C6  ...
pub fn draw_core_graphs(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    usage: &[f32],
    temps: &[Option<u32>],
) {
    if usage.is_empty() || height == 0 { return; }

    let cores = usage.len();
    let has_temps = !temps.is_empty();

    // 2-column layout with │ separator
    let cols = 2;
    let col_width = (width - 1) / cols;
    let rows_per_col = (cores + cols - 1) / cols;
    let actual_rows = rows_per_col.min(height);

    // Calculate what fits - dynamic meter width based on available space
    // Layout: label(4) + meter(dynamic) + pct(5) + space(1) + temp_meter(5) + temp(6) = 21 + meter
    let label_w = 4;
    let pct_w = 5;
    let temp_section_w = if has_temps { 1 + 5 + 6 } else { 0 }; // space + temp_meter + temp_value
    let fixed_w = label_w + pct_w + temp_section_w;

    let usage_meter_w = col_width.saturating_sub(fixed_w).max(5);
    let temp_meter_w = if has_temps { 5 } else { 0 };

    for row in 0..actual_rows {
        for col in 0..cols {
            let idx = col * rows_per_col + row;
            if idx >= cores { continue; }

            // Column separator between columns
            if col > 0 {
                term.set(x + col_width as i32, y + row as i32, '│', Some(Color::DarkGrey), false);
            }

            let cx = x + (col * (col_width + 1)) as i32;
            let cy = y + row as i32;
            let pct = usage[idx];
            let mut pos = cx;

            // Core label "C0  " padded to 4 chars
            let label = format!("{:<4}", format!("C{}", idx));
            term.set_str(pos, cy, &label, Some(Color::White), false);
            pos += label_w as i32;

            // Usage meter (linear style)
            if usage_meter_w > 0 {
                draw_meter_btop(term, pos, cy, usage_meter_w, pct, cpu_gradient_color(pct));
                pos += usage_meter_w as i32;
            }

            // Percentage "   0%" (5 chars, right-aligned)
            let pct_str = format!("{:4.0}%", pct);
            term.set_str(pos, cy, &pct_str, Some(cpu_gradient_color(pct)), false);
            pos += 5;

            // Temperature meter
            if temp_meter_w > 0 {
                pos += 1;
                if let Some(Some(temp)) = temps.get(idx) {
                    let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                    draw_meter_btop(term, pos, cy, temp_meter_w, temp_pct, temp_gradient_color(temp_pct));
                }
                pos += temp_meter_w as i32;
            }

            // Temperature value "  29°C" (2 spaces + 4 chars)
            if let Some(Some(temp)) = temps.get(idx) {
                let temp_str = format!("  {:2}°C", temp);
                let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                term.set_str(pos, cy, &temp_str, Some(temp_gradient_color(temp_pct)), false);
            }
        }
    }
}

/// Draw per-core meters with temps and color scheme support (btop-style with braille graphs)
/// Layout: C0⡇⡇⡇⡇⡇  9% ⡇⡇⡇⡇⡇ 45°C │ C6⡇⡇⡇⡇⡇ 12% ⡇⡇⡇⡇⡇ 54°C
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
    draw_core_graphs_with_history(term, x, y, width, height, usage, &[], temps, &[], colors);
}

/// Draw per-core graphs with history support (braille mini-graphs)
pub fn draw_core_graphs_with_history(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    usage: &[f32],
    usage_history: &[Vec<f32>],
    temps: &[Option<u32>],
    temp_history: &[Vec<f32>],
    colors: &ColorState,
) {
    if usage.is_empty() || height == 0 { return; }

    let cores = usage.len();
    let has_temps = !temps.is_empty();
    let has_history = !usage_history.is_empty();

    let cols = 2;
    let col_width = (width - 1) / cols;
    let rows_per_col = (cores + cols - 1) / cols;
    let actual_rows = rows_per_col.min(height);

    // Calculate b_column_size based on available width per column
    // btop adaptive layout:
    // b_column_size=2: full layout with both graphs
    // b_column_size=1: smaller usage graph, no temp graph
    // b_column_size=0: no graphs at all
    let min_with_both_graphs = 3 + 5 + 5 + 1 + 5 + 5;  // label(3) + graph(5) + pct(5) + sp(1) + temp_graph(5) + temp(5) = 24
    let min_with_usage_graph = 3 + 5 + 5 + 5;         // label(3) + graph(5) + pct(5) + temp(5) = 18
    let min_no_graphs = 3 + 5 + 5;                     // label(3) + pct(5) + temp(5) = 13

    let b_column_size = if col_width >= min_with_both_graphs && has_temps {
        2
    } else if col_width >= min_with_usage_graph {
        1
    } else if col_width >= min_no_graphs {
        0
    } else {
        0
    };

    // Fixed widths
    let label_w = 3;  // "C0" or "C12"
    let pct_w = 5;    // " 99%"
    let _temp_w = 5;  // " 45°C" (used for layout calculation)

    // Graph widths based on b_column_size
    let usage_graph_w = if b_column_size >= 1 {
        if b_column_size == 2 { 5 } else { 5 }
    } else { 0 };
    let temp_graph_w = if b_column_size >= 2 && has_temps { 5 } else { 0 };

    for row in 0..actual_rows {
        for col in 0..cols {
            let idx = col * rows_per_col + row;
            if idx >= cores { continue; }

            // Column separator
            if col > 0 {
                term.set(x + col_width as i32, y + row as i32, '│', Some(muted_color_scheme(colors)), false);
            }

            let cx = x + (col * (col_width + 1)) as i32;
            let cy = y + row as i32;
            let pct = usage[idx];
            let mut pos = cx;

            // Core label "C0" (no trailing space, btop-style)
            let label = format!("C{}", idx);
            term.set_str(pos, cy, &label, Some(text_color_scheme(colors)), false);
            pos += label_w as i32;

            // Usage graph (braille mini-graph if history available, otherwise meter)
            if usage_graph_w > 0 {
                if has_history && idx < usage_history.len() && !usage_history[idx].is_empty() {
                    draw_mini_graph_scheme(term, pos, cy, usage_graph_w, &usage_history[idx], colors, false);
                } else {
                    // Fallback: draw meter with current value
                    draw_meter_btop_scheme(term, pos, cy, usage_graph_w, pct, colors);
                }
                pos += usage_graph_w as i32;
            }

            // Percentage
            let pct_str = format!("{:4.0}%", pct);
            term.set_str(pos, cy, &pct_str, Some(cpu_gradient_color_scheme(pct, colors)), false);
            pos += pct_w as i32;

            // Temperature graph (braille mini-graph if history available)
            if temp_graph_w > 0 {
                if idx < temp_history.len() && !temp_history[idx].is_empty() {
                    draw_mini_graph_scheme(term, pos, cy, temp_graph_w, &temp_history[idx], colors, true);
                } else if let Some(Some(temp)) = temps.get(idx) {
                    let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                    draw_meter_btop_scheme(term, pos, cy, temp_graph_w, temp_pct, colors);
                }
                pos += temp_graph_w as i32;
            }

            // Temperature value
            if has_temps {
                if let Some(Some(temp)) = temps.get(idx) {
                    let temp_str = format!(" {:2}°C", temp);
                    let temp_pct = ((*temp as f32 - 20.0) / 80.0 * 100.0).clamp(0.0, 100.0);
                    term.set_str(pos, cy, &temp_str, Some(temp_gradient_color_scheme(temp_pct, colors)), false);
                }
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

/// Color gradient based on percentage (btop-style, TTY colors)
pub fn gradient_color(percent: f32) -> Color {
    if percent >= 90.0 {
        Color::Red
    } else if percent >= 80.0 {
        Color::DarkYellow  // Orange equivalent
    } else if percent >= 60.0 {
        Color::Yellow
    } else if percent >= 40.0 {
        Color::Green
    } else {
        Color::Cyan
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

/// Get gradient color with scheme support
pub fn gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    if colors.is_mono() {
        gradient_color(percent)
    } else {
        let intensity = if percent >= 80.0 { 3 } else if percent >= 50.0 { 2 } else { 1 };
        scheme_color(colors.scheme, intensity, percent >= 80.0).0
    }
}

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

// Braille characters for mini vertical bar graphs (left column)
// Height maps to these characters: 0/4, 1/4, 2/4, 3/4, 4/4
const BRAILLE_UP: [char; 5] = [
    '⠀', // 0/4 - empty (0x2800)
    '⡀', // 1/4 - dot 7 (0x2840)
    '⡄', // 2/4 - dots 3,7 (0x2844)
    '⡆', // 3/4 - dots 2,3,7 (0x2846)
    '⡇', // 4/4 - dots 1,2,3,7 (0x2847)
];

/// Background character for braille graphs (btop uses bottom dots)
const BRAILLE_BG: char = '⣀'; // dots 7,8 (0x28C0) - "floor" character

/// Draw a mini braille graph showing history values
/// Each character represents one sample, height based on percentage
pub fn draw_mini_graph(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    history: &[f32],
    fg_color: Color,
    bg_color: Color,
) {
    if width == 0 { return; }

    // Draw background first
    for i in 0..width {
        term.set(x + i as i32, y, BRAILLE_BG, Some(bg_color), false);
    }

    // Draw history values (most recent on the right)
    let start_idx = if history.len() > width {
        history.len() - width
    } else {
        0
    };

    for (i, &val) in history.iter().skip(start_idx).enumerate() {
        if i >= width { break; }
        // Map percentage (0-100) to braille height (0-4)
        let height_idx = ((val / 100.0) * 4.0).round() as usize;
        let height_idx = height_idx.clamp(0, 4);

        if height_idx > 0 {
            term.set(x + i as i32, y, BRAILLE_UP[height_idx], Some(fg_color), false);
        }
    }
}

/// Draw mini graph with color scheme support
pub fn draw_mini_graph_scheme(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    history: &[f32],
    colors: &ColorState,
    is_temp: bool,
) {
    if width == 0 { return; }

    let bg_color = muted_color_scheme(colors);

    // Draw background first
    for i in 0..width {
        term.set(x + i as i32, y, BRAILLE_BG, Some(bg_color), false);
    }

    // Draw history values
    let start_idx = if history.len() > width {
        history.len() - width
    } else {
        0
    };

    for (i, &val) in history.iter().skip(start_idx).enumerate() {
        if i >= width { break; }
        let height_idx = ((val / 100.0) * 4.0).round() as usize;
        let height_idx = height_idx.clamp(0, 4);

        if height_idx > 0 {
            // Color based on the value - use gradient
            let fg_color = if is_temp {
                temp_gradient_color_scheme(val, colors)
            } else {
                cpu_gradient_color_scheme(val, colors)
            };
            term.set(x + i as i32, y, BRAILLE_UP[height_idx], Some(fg_color), false);
        }
    }
}


