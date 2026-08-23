use crate::colors::{scheme_color, ColorState};
use crate::terminal::Terminal;
use crossterm::style::Color;
use std::collections::VecDeque;

/// A bounding box for layout calculations
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    /// Inner content area (excluding borders)
    pub fn inner_x(&self) -> i32 {
        self.x + 1
    }
    pub fn inner_y(&self) -> i32 {
        self.y + 1
    }
    pub fn inner_width(&self) -> u16 {
        self.width.saturating_sub(2)
    }
    pub fn inner_height(&self) -> u16 {
        self.height.saturating_sub(2)
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
    if width == 0 {
        return;
    }

    const METER_CHAR: char = '■';
    let filled = ((percent / 100.0) * width as f32) as usize;

    for i in 0..width {
        if i < filled {
            let pos_pct = (i as f32 / width as f32) * 100.0;
            let grad = cpu_gradient_color_scheme(pos_pct.min(percent), colors);
            term.set(x + i as i32, y, METER_CHAR, Some(grad), false);
        } else {
            term.set(
                x + i as i32,
                y,
                METER_CHAR,
                Some(muted_color_scheme(colors)),
                false,
            );
        }
    }
}

/// Draw a meter whose fill reads as headroom rather than load: a full bar is
/// healthy (green), an empty one is critical (red). The fill is a single color
/// so the row reports a state instead of a left-to-right severity ramp.
pub fn draw_meter_headroom_scheme(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    colors: &ColorState,
) {
    if width == 0 {
        return;
    }

    const METER_CHAR: char = '■';
    let filled = ((percent / 100.0) * width as f32) as usize;
    let fill_color = headroom_gradient_color_scheme(percent, colors);
    let empty_color = muted_color_scheme(colors);

    for i in 0..width {
        let color = if i < filled { fill_color } else { empty_color };
        term.set(x + i as i32, y, METER_CHAR, Some(color), false);
    }
}

/// Vertical block glyphs from lowest to highest fill. Eight levels give a
/// single-row graph enough resolution to read as a trend rather than a step.
const SPARK_GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Columns an inline history graph occupies inside a metric row.
const HISTORY_GRAPH_WIDTH: usize = 16;

/// Blank column separating a graph from the meter to its left.
const HISTORY_GRAPH_GAP: usize = 1;

/// Meter columns a row keeps before it may spend any width on a graph.
const MIN_GRAPHED_METER_WIDTH: usize = 8;

/// Samples retained per graphed metric. Only the newest `HISTORY_GRAPH_WIDTH`
/// of them reach the screen; the slack bounds memory whilst leaving room for a
/// wider graph without revisiting the sampling path.
pub const HISTORY_CAPACITY: usize = 32;

/// A fixed-capacity ring of recent samples.
///
/// The capacity is fixed at construction and pushing past it evicts the oldest
/// sample, so a panel that has run for days holds no more history than one that
/// has just started.
pub struct SampleHistory {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl SampleHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Record the newest sample, evicting the oldest ones once full. A capacity
    /// of zero retains nothing.
    pub fn push(&mut self, value: f32) {
        if self.capacity == 0 {
            return;
        }

        while self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Samples in the order they were recorded, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        self.samples.iter().copied()
    }
}

/// Map a history onto `width` columns with the newest sample in the rightmost
/// one. A buffer holding fewer samples than there are columns renders flush
/// right and leaves the left remainder empty; a fuller one drops its oldest
/// samples off the left edge.
fn history_columns(history: &SampleHistory, width: usize) -> Vec<Option<f32>> {
    let shown = history.len().min(width);
    let mut columns = vec![None; width - shown];
    columns.extend(history.iter().skip(history.len() - shown).map(Some));
    columns
}

/// Pick the block glyph reporting a percentage on a single row.
fn spark_glyph(percent: f32) -> char {
    let level = (percent.clamp(0.0, 100.0) / 100.0 * SPARK_GLYPHS.len() as f32) as usize;
    SPARK_GLYPHS[level.min(SPARK_GLYPHS.len() - 1)]
}

/// Somewhere colored glyphs land one cell at a time. `Terminal` is the live
/// surface; tests substitute a recorder so what a renderer actually drew can
/// be asserted without a tty.
pub trait GlyphSink {
    fn put(&mut self, x: i32, y: i32, glyph: char, color: Color);
}

impl GlyphSink for Terminal {
    fn put(&mut self, x: i32, y: i32, glyph: char, color: Color) {
        self.set(x, y, glyph, Some(color), false);
    }
}

/// Draw a right-anchored single-row history graph: the newest sample occupies
/// the rightmost column and older samples extend leftward.
///
/// `tint` maps a sample to its color so callers reuse an existing gradient
/// instead of restating its thresholds. A zero width or an empty history draws
/// nothing, leaving the row reading exactly as it did before the first sample.
pub fn draw_history_graph_scheme(
    sink: &mut impl GlyphSink,
    x: i32,
    y: i32,
    width: usize,
    history: &SampleHistory,
    tint: fn(f32, &ColorState) -> Color,
    colors: &ColorState,
) {
    if width == 0 || history.is_empty() {
        return;
    }

    let empty_color = muted_color_scheme(colors);
    for (i, sample) in history_columns(history, width).into_iter().enumerate() {
        let (glyph, color) = match sample {
            Some(percent) => (spark_glyph(percent), tint(percent, colors)),
            None => (SPARK_GLYPHS[0], empty_color),
        };
        sink.put(x + i as i32, y, glyph, color);
    }
}

/// Split a metric row's elastic span into meter columns and graph columns.
///
/// The graph is dropped whole rather than squeezed in beside a stub of a meter,
/// so a row too narrow to afford one renders exactly as it did before graphs
/// existed. A zero graph width means no graph.
pub fn split_row_for_graph(elastic: usize) -> (usize, usize) {
    let cost = HISTORY_GRAPH_GAP + HISTORY_GRAPH_WIDTH;
    match elastic.checked_sub(cost) {
        Some(meter) if meter >= MIN_GRAPHED_METER_WIDTH => (meter, HISTORY_GRAPH_WIDTH),
        _ => (elastic, 0),
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
    if usage.is_empty() || height == 0 {
        return;
    }

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
            if idx >= cores {
                continue;
            }

            if col > 0 {
                term.set(
                    x + col_width as i32,
                    y + row as i32,
                    '│',
                    Some(muted_color_scheme(colors)),
                    false,
                );
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
            term.set_str(
                pos,
                cy,
                &pct_str,
                Some(cpu_gradient_color_scheme(pct, colors)),
                false,
            );
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
                term.set_str(
                    pos,
                    cy,
                    &temp_str,
                    Some(temp_gradient_color_scheme(temp_pct, colors)),
                    false,
                );
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

/// Map a byte rate onto a logarithmic activity scale.
///
/// Rate meters do not represent hardware utilization because link and device
/// capacities are generally unknown. A logarithmic scale keeps background
/// activity visible whilst the adjacent rate remains the precise measurement.
pub fn activity_percent(rate: f64, scale: f64) -> f32 {
    if !rate.is_finite() || !scale.is_finite() || rate <= 0.0 || scale <= 0.0 {
        return 0.0;
    }

    let rate_kib = rate / 1024.0;
    let scale_kib = scale.max(rate) / 1024.0;
    (rate_kib.ln_1p() / scale_kib.ln_1p() * 100.0).clamp(0.0, 100.0) as f32
}

/// Update an adaptive activity scale using elapsed-time-based decay.
pub fn update_activity_scale(previous: f64, current: f64, elapsed_secs: f32, floor: f64) -> f64 {
    const HALF_LIFE_SECS: f64 = 30.0;

    let elapsed = elapsed_secs.max(0.0) as f64;
    let decay = 0.5_f64.powf(elapsed / HALF_LIFE_SECS);
    current.max((previous * decay).max(floor))
}

/// CPU mini graph gradient (btop TTY: bright green -> bright red based on VALUE)
/// Maps percentage 0-100 to ANSI bright green (10) → bright yellow (11) → bright red (9)
pub fn cpu_gradient_color(percent: f32) -> Color {
    if percent >= 80.0 {
        Color::AnsiValue(9) // Bright red (ANSI 91)
    } else if percent >= 50.0 {
        Color::AnsiValue(11) // Bright yellow (ANSI 93)
    } else {
        Color::AnsiValue(10) // Bright green (ANSI 92)
    }
}

/// Temperature mini graph gradient (btop TTY: bright blue -> bright magenta based on VALUE)
/// Maps temperature percentage to ANSI bright blue (12) → bright magenta (13)
pub fn temp_gradient_color(percent: f32) -> Color {
    if percent >= 70.0 {
        Color::AnsiValue(13) // Bright magenta (ANSI 95)
    } else if percent >= 40.0 {
        Color::AnsiValue(14) // Bright cyan (ANSI 96) - mid point
    } else {
        Color::AnsiValue(12) // Bright blue (ANSI 94)
    }
}

// ============ Scheme-aware color functions ============

/// Get CPU gradient color with scheme support
pub fn cpu_gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    if colors.is_mono() {
        cpu_gradient_color(percent)
    } else {
        let intensity = if percent >= 80.0 {
            3
        } else if percent >= 50.0 {
            2
        } else {
            1
        };
        scheme_color(colors.scheme, intensity, percent >= 80.0).0
    }
}

/// Get gradient color for a headroom value, where a high percentage is healthy.
/// Inverts the usage gradient: >=50% available is green, <20% is red.
pub fn headroom_gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    cpu_gradient_color_scheme(100.0 - percent.clamp(0.0, 100.0), colors)
}

/// Get temperature gradient color with scheme support
pub fn temp_gradient_color_scheme(percent: f32, colors: &ColorState) -> Color {
    if colors.is_mono() {
        temp_gradient_color(percent)
    } else {
        let intensity = if percent >= 70.0 {
            3
        } else if percent >= 40.0 {
            2
        } else {
            1
        };
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

#[cfg(test)]
mod tests {
    use super::{
        activity_percent, draw_history_graph_scheme, headroom_gradient_color_scheme,
        history_columns, muted_color_scheme, spark_glyph, split_row_for_graph,
        update_activity_scale, GlyphSink, SampleHistory, HISTORY_GRAPH_GAP, HISTORY_GRAPH_WIDTH,
        MIN_GRAPHED_METER_WIDTH,
    };
    use crate::colors::ColorState;
    use crossterm::style::Color;

    /// Records every cell a renderer draws, standing in for `Terminal`.
    struct RecordingSink(Vec<(i32, i32, char, Color)>);

    impl GlyphSink for RecordingSink {
        fn put(&mut self, x: i32, y: i32, glyph: char, color: Color) {
            self.0.push((x, y, glyph, color));
        }
    }

    #[test]
    fn activity_scale_keeps_low_rates_visible() {
        let pct = activity_percent(53.0 * 1024.0, 1024.0 * 1024.0);
        assert!(pct > 50.0 && pct < 60.0);
        assert_eq!(activity_percent(0.0, 1024.0), 0.0);
    }

    #[test]
    fn activity_scale_decay_is_time_based_and_floored() {
        let mib = 1024.0 * 1024.0;
        let decayed = update_activity_scale(8.0 * mib, 0.0, 30.0, mib);
        assert!((decayed - 4.0 * mib).abs() < 1.0);
        assert_eq!(update_activity_scale(mib, 0.0, 300.0, mib), mib);
    }

    #[test]
    fn headroom_gradient_is_green_when_free_and_red_when_scarce() {
        let mono = ColorState::new(7);
        assert_eq!(
            headroom_gradient_color_scheme(70.0, &mono),
            Color::AnsiValue(10)
        );
        assert_eq!(
            headroom_gradient_color_scheme(35.0, &mono),
            Color::AnsiValue(11)
        );
        assert_eq!(
            headroom_gradient_color_scheme(5.0, &mono),
            Color::AnsiValue(9)
        );
    }

    #[test]
    fn history_graph_puts_the_newest_sample_in_the_rightmost_column() {
        let mut history = SampleHistory::new(8);
        for value in [10.0, 20.0, 30.0, 40.0] {
            history.push(value);
        }

        // Wider than the sample count, so a left-anchored graph would differ:
        // it would pad on the right instead of the left.
        let columns = history_columns(&history, 6);
        assert_eq!(
            columns,
            vec![None, None, Some(10.0), Some(20.0), Some(30.0), Some(40.0)]
        );
        assert_eq!(columns.last().copied().flatten(), Some(40.0));
    }

    #[test]
    fn draw_history_graph_writes_right_anchored_cells_through_the_sink() {
        let mono = ColorState::new(7);
        let mut history = SampleHistory::new(8);
        for value in [10.0, 55.0, 95.0] {
            history.push(value);
        }

        let mut sink = RecordingSink(Vec::new());
        draw_history_graph_scheme(
            &mut sink,
            4,
            2,
            6,
            &history,
            headroom_gradient_color_scheme,
            &mono,
        );

        // Three muted filler cells on the left, then the samples, oldest to
        // newest, each tinted by headroom and ending flush right at x=9.
        let muted = muted_color_scheme(&mono);
        assert_eq!(
            sink.0,
            vec![
                (4, 2, '\u{2581}', muted),
                (5, 2, '\u{2581}', muted),
                (6, 2, '\u{2581}', muted),
                (7, 2, '\u{2581}', Color::AnsiValue(9)),
                (8, 2, '\u{2585}', Color::AnsiValue(10)),
                (9, 2, '\u{2588}', Color::AnsiValue(10)),
            ]
        );
    }

    #[test]
    fn draw_history_graph_draws_nothing_for_zero_width_or_empty_history() {
        let mono = ColorState::new(7);
        let mut history = SampleHistory::new(4);
        history.push(50.0);

        let mut sink = RecordingSink(Vec::new());
        draw_history_graph_scheme(
            &mut sink,
            0,
            0,
            0,
            &history,
            headroom_gradient_color_scheme,
            &mono,
        );
        draw_history_graph_scheme(
            &mut sink,
            0,
            0,
            6,
            &SampleHistory::new(4),
            headroom_gradient_color_scheme,
            &mono,
        );

        assert!(sink.0.is_empty());
    }

    #[test]
    fn history_graph_renders_a_partial_buffer_flush_right() {
        let mut history = SampleHistory::new(8);
        history.push(25.0);
        history.push(75.0);

        assert_eq!(
            history_columns(&history, 5),
            vec![None, None, None, Some(25.0), Some(75.0)]
        );
    }

    #[test]
    fn history_graph_drops_the_oldest_samples_off_the_left_edge() {
        let mut history = SampleHistory::new(8);
        for value in [1.0, 2.0, 3.0, 4.0] {
            history.push(value);
        }

        assert_eq!(history_columns(&history, 2), vec![Some(3.0), Some(4.0)]);
        assert!(history_columns(&history, 0).is_empty());
    }

    #[test]
    fn sample_history_evicts_the_oldest_at_capacity() {
        let mut history = SampleHistory::new(3);
        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            history.push(value);
        }
        assert_eq!(history.len(), 3);
        assert_eq!(history.iter().collect::<Vec<_>>(), vec![3.0, 4.0, 5.0]);

        let mut single = SampleHistory::new(1);
        single.push(1.0);
        single.push(2.0);
        assert_eq!(single.iter().collect::<Vec<_>>(), vec![2.0]);

        let mut nothing = SampleHistory::new(0);
        nothing.push(1.0);
        assert!(nothing.is_empty());
    }

    #[test]
    fn spark_glyphs_span_the_percentage_range_without_panicking() {
        assert_eq!(spark_glyph(0.0), '\u{2581}');
        assert_eq!(spark_glyph(50.0), '\u{2585}');
        assert_eq!(spark_glyph(100.0), '\u{2588}');
        assert_eq!(spark_glyph(150.0), '\u{2588}');
        assert_eq!(spark_glyph(-10.0), '\u{2581}');
        assert_eq!(spark_glyph(f32::NAN), '\u{2581}');
    }

    #[test]
    fn narrow_rows_drop_the_graph_instead_of_the_meter() {
        let cost = HISTORY_GRAPH_GAP + HISTORY_GRAPH_WIDTH;
        let widest_without_graph = cost + MIN_GRAPHED_METER_WIDTH - 1;

        assert_eq!(split_row_for_graph(0), (0, 0));
        assert_eq!(
            split_row_for_graph(widest_without_graph),
            (widest_without_graph, 0)
        );
        assert_eq!(
            split_row_for_graph(widest_without_graph + 1),
            (MIN_GRAPHED_METER_WIDTH, HISTORY_GRAPH_WIDTH)
        );
    }

    #[test]
    fn headroom_gradient_bands_switch_at_their_boundaries() {
        let mono = ColorState::new(7);

        assert_eq!(
            headroom_gradient_color_scheme(0.0, &mono),
            Color::AnsiValue(9)
        );
        assert_eq!(
            headroom_gradient_color_scheme(20.0, &mono),
            Color::AnsiValue(9)
        );
        assert_eq!(
            headroom_gradient_color_scheme(20.1, &mono),
            Color::AnsiValue(11)
        );
        assert_eq!(
            headroom_gradient_color_scheme(50.0, &mono),
            Color::AnsiValue(11)
        );
        assert_eq!(
            headroom_gradient_color_scheme(50.1, &mono),
            Color::AnsiValue(10)
        );
        assert_eq!(
            headroom_gradient_color_scheme(100.0, &mono),
            Color::AnsiValue(10)
        );
    }
}
