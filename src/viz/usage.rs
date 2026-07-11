use crate::colors::ColorState;
use crate::monitor::layout::{cpu_gradient_color_scheme, muted_color_scheme};
use crate::terminal::Terminal;
use crossterm::style::Color;
use std::time::Duration;

pub(super) fn elapsed_percent(remaining: Duration, window: Duration) -> f64 {
    if window.is_zero() {
        return 0.0;
    }

    let elapsed = window.saturating_sub(remaining);
    (elapsed.as_secs_f64() / window.as_secs_f64() * 100.0).clamp(0.0, 100.0)
}

pub(super) fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

pub(super) fn format_window(window_secs: u64) -> String {
    if window_secs > 0 && window_secs.is_multiple_of(86_400) {
        let days = window_secs / 86_400;
        if days == 7 {
            "7-Day".to_string()
        } else {
            format!("{}-Day", days)
        }
    } else if window_secs > 0 && window_secs.is_multiple_of(3_600) {
        format!("{}-Hour", window_secs / 3_600)
    } else if window_secs > 0 && window_secs.is_multiple_of(60) {
        format!("{}-Min", window_secs / 60)
    } else {
        "Usage".to_string()
    }
}

pub(super) fn text_columns(text: &str) -> usize {
    text.chars().count()
}

/// Draw a quota bar with an optional pacing underlay.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_usage_bar(
    term: &mut Terminal,
    x: usize,
    y: usize,
    width: usize,
    pct: f64,
    expected_pct: Option<f64>,
    label: &str,
    colors: &ColorState,
) {
    const LABEL_WIDTH: usize = 8;
    const PERCENT_WIDTH: usize = 6;

    let meter_width = width.saturating_sub(LABEL_WIDTH + PERCENT_WIDTH);
    let mut pos = x as i32;
    let label: String = label.chars().take(LABEL_WIDTH).collect();

    term.set_str(
        pos,
        y as i32,
        &format!("{:<LABEL_WIDTH$}", label),
        Some(muted_color_scheme(colors)),
        false,
    );
    pos += LABEL_WIDTH as i32;

    draw_meter_with_pacing(
        term,
        pos,
        y as i32,
        meter_width,
        pct.clamp(0.0, 100.0) as f32,
        expected_pct.map(|value| value.clamp(0.0, 100.0) as f32),
        colors,
    );
    pos += meter_width as i32;

    let pct = pct.clamp(0.0, 100.0);
    let pct_str = format!("{:5.1}%", pct);
    let color = cpu_gradient_color_scheme(pct as f32, colors);
    term.set_str(pos, y as i32, &pct_str, Some(color), pct >= 80.0);
}

fn draw_meter_with_pacing(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    expected_pct: Option<f32>,
    colors: &ColorState,
) {
    if width == 0 {
        return;
    }

    const METER_CHAR: char = '■';
    let filled = ((percent / 100.0) * width as f32) as usize;
    let expected_filled = expected_pct
        .map(|expected| ((expected / 100.0) * width as f32) as usize)
        .unwrap_or(0);
    let ghost_color = Color::AnsiValue(238);

    for i in 0..width {
        let color = if i < filled {
            let position = (i as f32 / width as f32) * 100.0;
            cpu_gradient_color_scheme(position.min(percent), colors)
        } else if i < expected_filled {
            ghost_color
        } else {
            muted_color_scheme(colors)
        };
        term.set(x + i as i32, y, METER_CHAR, Some(color), false);
    }
}

#[cfg(test)]
mod tests {
    use super::{elapsed_percent, format_duration, format_window};
    use std::time::Duration;

    #[test]
    fn quota_time_formatting_is_shared_across_providers() {
        assert_eq!(format_window(18_000), "5-Hour");
        assert_eq!(format_window(604_800), "7-Day");
        assert_eq!(format_duration(Duration::from_secs(90_000)), "1d 1h");
        assert_eq!(
            elapsed_percent(Duration::from_secs(15), Duration::from_secs(60)),
            75.0
        );
    }
}
