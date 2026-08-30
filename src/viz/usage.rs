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

/// Render a reset countdown in the compact fixed grammar shared by every token
/// meter: `2D04H` from a day out, where minutes are never shown beside days;
/// `04H37M` within the day; `04H` when the minute field would be zero; and
/// `37M` within the hour. Uppercase units, no spaces, no prefix.
pub(super) fn format_countdown(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;

    if days > 0 {
        format!("{}D{:02}H", days, hours)
    } else if hours > 0 && mins > 0 {
        format!("{:02}H{:02}M", hours, mins)
    } else if hours > 0 {
        format!("{:02}H", hours)
    } else {
        format!("{:02}M", mins)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuotaColorBand {
    OnPace,
    NearBoundary,
    OverBoundary,
}

const PACING_WARNING_MARGIN: f32 = 15.0;
const DEFAULT_WARNING_THRESHOLD: f32 = 50.0;
const DEFAULT_OVERUSE_THRESHOLD: f32 = 80.0;

fn quota_color_band(pct: f32, expected_pct: Option<f32>) -> QuotaColorBand {
    if let Some(boundary) = expected_pct {
        if pct > boundary {
            QuotaColorBand::OverBoundary
        } else if pct >= (boundary - PACING_WARNING_MARGIN).max(0.0) {
            QuotaColorBand::NearBoundary
        } else {
            QuotaColorBand::OnPace
        }
    } else if pct > DEFAULT_OVERUSE_THRESHOLD {
        QuotaColorBand::OverBoundary
    } else if pct >= DEFAULT_WARNING_THRESHOLD {
        QuotaColorBand::NearBoundary
    } else {
        QuotaColorBand::OnPace
    }
}

fn quota_band_color(band: QuotaColorBand, colors: &ColorState) -> Color {
    let gradient_value = match band {
        QuotaColorBand::OnPace => 0.0,
        QuotaColorBand::NearBoundary => 50.0,
        QuotaColorBand::OverBoundary => 80.0,
    };
    cpu_gradient_color_scheme(gradient_value, colors)
}

fn meter_color_band(index: usize, width: usize, expected_pct: Option<f32>) -> QuotaColorBand {
    let position = (index as f32 / width as f32) * 100.0;
    quota_color_band(position, expected_pct)
}

/// Fixed column budgets for one meter row. Label and percent are the existing
/// layout; the countdown column is one separating space plus the widest
/// countdown the grammar produces for a real quota window (`04H37M`, `12D04H`).
const LABEL_WIDTH: usize = 8;
const PERCENT_WIDTH: usize = 6;
const COUNTDOWN_WIDTH: usize = 7;
const COUNTDOWN_TEXT_WIDTH: usize = COUNTDOWN_WIDTH - 1;

/// Columns reserved for the countdown on a row `width` columns wide. The budget
/// is only claimed when the row can hold every fixed column, so a pane too
/// narrow for a countdown keeps its previous label/meter/percent layout rather
/// than pushing the countdown outside the bar box.
fn countdown_budget(width: usize) -> usize {
    if width >= LABEL_WIDTH + PERCENT_WIDTH + COUNTDOWN_WIDTH {
        COUNTDOWN_WIDTH
    } else {
        0
    }
}

/// Columns left for the meter once the fixed columns are reserved. The
/// countdown budget comes out of the meter instead of being appended, so a row
/// is exactly `width` columns wide and the percent column lands in the same
/// place for every entry in a pane.
fn meter_width(width: usize) -> usize {
    width.saturating_sub(LABEL_WIDTH + PERCENT_WIDTH + countdown_budget(width))
}

/// Column offset from the start of the row at which the countdown text begins,
/// or `None` when the row cannot hold every fixed column. The drawing code and
/// the tests read the position from this one expression, so the invariant that
/// matters -- `offset + COUNTDOWN_TEXT_WIDTH == width`, i.e. the row ends on its
/// last column and never past it -- is proven rather than assumed.
fn countdown_offset(width: usize) -> Option<usize> {
    if countdown_budget(width) == 0 {
        return None;
    }
    Some(LABEL_WIDTH + meter_width(width) + PERCENT_WIDTH + 1)
}

/// Draw a quota bar with an optional pacing underlay and reset countdown.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_usage_bar(
    term: &mut Terminal,
    x: usize,
    y: usize,
    width: usize,
    pct: f64,
    expected_pct: Option<f64>,
    countdown: Option<Duration>,
    label: &str,
    colors: &ColorState,
) {
    let meter_width = meter_width(width);
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
    let band = quota_color_band(
        pct as f32,
        expected_pct.map(|value| value.clamp(0.0, 100.0) as f32),
    );
    let color = quota_band_color(band, colors);
    term.set_str(
        pos,
        y as i32,
        &pct_str,
        Some(color),
        band == QuotaColorBand::OverBoundary,
    );

    let Some((offset, countdown)) = countdown_offset(width).zip(countdown) else {
        return;
    };
    // Truncated to the reserved text budget so an implausible countdown can
    // never push the row past `width`.
    let countdown: String = format_countdown(countdown)
        .chars()
        .take(COUNTDOWN_TEXT_WIDTH)
        .collect();
    term.set_str(
        x as i32 + offset as i32,
        y as i32,
        &countdown,
        Some(muted_color_scheme(colors)),
        false,
    );
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
            quota_band_color(meter_color_band(i, width, expected_pct), colors)
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
    use super::{
        countdown_budget, countdown_offset, elapsed_percent, format_countdown, format_window,
        meter_color_band, meter_width, quota_band_color, quota_color_band, QuotaColorBand,
        COUNTDOWN_TEXT_WIDTH, COUNTDOWN_WIDTH, LABEL_WIDTH, PERCENT_WIDTH,
    };
    use crate::colors::ColorState;
    use crossterm::style::Color;
    use std::time::Duration;

    #[test]
    fn quota_time_formatting_is_shared_across_providers() {
        assert_eq!(format_window(18_000), "5-Hour");
        assert_eq!(format_window(604_800), "7-Day");
        assert_eq!(format_countdown(Duration::from_secs(90_000)), "1D01H");
        assert_eq!(
            elapsed_percent(Duration::from_secs(15), Duration::from_secs(60)),
            75.0
        );
    }

    const MIN: u64 = 60;
    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;

    #[test]
    fn countdown_shows_days_and_padded_hours_from_a_day_out() {
        assert_eq!(
            format_countdown(Duration::from_secs(2 * DAY + 4 * HOUR + 37 * MIN)),
            "2D04H"
        );
        // Minutes are never shown beside days, however large they are.
        assert_eq!(
            format_countdown(Duration::from_secs(12 * DAY + 59 * MIN)),
            "12D00H"
        );
        assert_eq!(
            format_countdown(Duration::from_secs(7 * DAY + 9 * HOUR + 59 * MIN)),
            "7D09H"
        );
    }

    #[test]
    fn countdown_shows_padded_hours_and_minutes_within_a_day() {
        assert_eq!(
            format_countdown(Duration::from_secs(4 * HOUR + 37 * MIN)),
            "04H37M"
        );
        assert_eq!(
            format_countdown(Duration::from_secs(23 * HOUR + 5 * MIN)),
            "23H05M"
        );
    }

    #[test]
    fn countdown_drops_the_minute_field_when_minutes_are_zero() {
        assert_eq!(format_countdown(Duration::from_secs(4 * HOUR)), "04H");
        assert_eq!(format_countdown(Duration::from_secs(HOUR + 59)), "01H");
        assert_eq!(format_countdown(Duration::from_secs(23 * HOUR)), "23H");
    }

    #[test]
    fn countdown_shows_minutes_alone_within_the_hour() {
        assert_eq!(format_countdown(Duration::from_secs(37 * MIN)), "37M");
        assert_eq!(format_countdown(Duration::from_secs(5 * MIN)), "05M");
        assert_eq!(format_countdown(Duration::ZERO), "00M");
    }

    #[test]
    fn countdown_switches_form_exactly_on_the_branch_boundaries() {
        // One second under a day is still hours-and-minutes; the day itself
        // switches to days-and-hours and drops the minute field.
        assert_eq!(format_countdown(Duration::from_secs(DAY - 1)), "23H59M");
        assert_eq!(format_countdown(Duration::from_secs(DAY)), "1D00H");
        // One second under an hour is still minutes-only; the hour itself
        // switches to the hour field with no minutes.
        assert_eq!(format_countdown(Duration::from_secs(HOUR - 1)), "59M");
        assert_eq!(format_countdown(Duration::from_secs(HOUR)), "01H");
        // One second under a minute rounds down to a zero-minute countdown.
        assert_eq!(format_countdown(Duration::from_secs(MIN - 1)), "00M");
        assert_eq!(format_countdown(Duration::from_secs(MIN)), "01M");
    }

    #[test]
    fn every_countdown_fits_the_reserved_column() {
        // The rendering depends only on the whole-minute count, so stepping a
        // minute at a time over the whole span of a real quota window covers
        // every distinct string the grammar can produce there. Sampling would
        // leave the widest cases to luck; this leaves none.
        for minute in 0..=(8 * DAY / MIN) {
            let text = format_countdown(Duration::from_secs(minute * MIN));
            assert!(
                text.chars().count() <= COUNTDOWN_TEXT_WIDTH,
                "{} is wider than the countdown column",
                text
            );
        }
    }

    /// Exclusive end column of the widest element a row draws, measured from
    /// the start of the row. Mirrors the order `draw_usage_bar` writes in:
    /// label, meter, percent, and -- when the budget is claimed and the entry
    /// has one -- the countdown.
    fn row_end_column(width: usize, has_countdown: bool) -> usize {
        match countdown_offset(width) {
            Some(offset) if has_countdown => offset + COUNTDOWN_TEXT_WIDTH,
            _ => LABEL_WIDTH + meter_width(width) + PERCENT_WIDTH,
        }
    }

    #[test]
    fn no_row_element_is_drawn_outside_the_bar_box() {
        const RESERVED: usize = LABEL_WIDTH + PERCENT_WIDTH + COUNTDOWN_WIDTH;

        for width in 0..=200usize {
            for has_countdown in [false, true] {
                let end = row_end_column(width, has_countdown);
                if width >= LABEL_WIDTH + PERCENT_WIDTH {
                    assert!(
                        end <= width,
                        "width {} with countdown {} ends at column {}",
                        width,
                        has_countdown,
                        end
                    );
                } else {
                    // Below the label-plus-percent minimum the percent column
                    // already overhung the box before this change; the row is
                    // byte-for-byte what the baseline drew, not a new overrun.
                    assert_eq!(end, LABEL_WIDTH + PERCENT_WIDTH, "width {}", width);
                }
            }
        }

        // Where the countdown is drawn it fills the row exactly to its last
        // column: one column further would leave the box, one column fewer
        // would mean the budget carved out of the meter was never used.
        for width in RESERVED..=200usize {
            assert_eq!(row_end_column(width, true), width, "width {}", width);
        }
    }

    #[test]
    fn countdown_budget_is_taken_from_the_meter_not_the_row() {
        const RESERVED: usize = LABEL_WIDTH + PERCENT_WIDTH + COUNTDOWN_WIDTH;

        // The row stays `width` columns wide: the countdown column is carved
        // out of the meter, so no row overruns the bar box.
        assert_eq!(countdown_budget(60), COUNTDOWN_WIDTH);
        assert_eq!(meter_width(60) + RESERVED, 60);
        assert_eq!(countdown_budget(RESERVED), COUNTDOWN_WIDTH);
        assert_eq!(meter_width(RESERVED), 0);
    }

    #[test]
    fn rows_too_narrow_for_a_countdown_keep_the_previous_layout() {
        const RESERVED: usize = LABEL_WIDTH + PERCENT_WIDTH + COUNTDOWN_WIDTH;

        // Below the threshold the countdown is dropped rather than drawn past
        // the bar box, and the meter keeps every column it had before. The
        // gate is at 21, so this covers the widths 14 through 20 where the
        // meter would otherwise have been squeezed to nothing.
        for width in 0..RESERVED {
            assert_eq!(countdown_budget(width), 0, "width {}", width);
            assert_eq!(countdown_offset(width), None, "width {}", width);
            assert_eq!(
                meter_width(width),
                width.saturating_sub(LABEL_WIDTH + PERCENT_WIDTH),
                "width {}",
                width
            );
        }
        // The first width that can hold every fixed column is the first that
        // draws a countdown, and it starts immediately after the percent
        // column and its separating space.
        assert_eq!(countdown_budget(RESERVED), COUNTDOWN_WIDTH);
        assert_eq!(
            countdown_offset(RESERVED),
            Some(LABEL_WIDTH + PERCENT_WIDTH + 1)
        );
    }

    #[test]
    fn quota_color_warns_before_and_errors_after_the_pacing_boundary() {
        assert_eq!(quota_color_band(24.9, Some(40.0)), QuotaColorBand::OnPace);
        assert_eq!(
            quota_color_band(25.0, Some(40.0)),
            QuotaColorBand::NearBoundary
        );
        assert_eq!(
            quota_color_band(40.0, Some(40.0)),
            QuotaColorBand::NearBoundary
        );
        assert_eq!(
            quota_color_band(40.1, Some(40.0)),
            QuotaColorBand::OverBoundary
        );
    }

    #[test]
    fn quota_color_preserves_legacy_threshold_without_pacing_data() {
        assert_eq!(quota_color_band(49.9, None), QuotaColorBand::OnPace);
        assert_eq!(quota_color_band(50.0, None), QuotaColorBand::NearBoundary);
        assert_eq!(quota_color_band(80.0, None), QuotaColorBand::NearBoundary);
        assert_eq!(quota_color_band(80.1, None), QuotaColorBand::OverBoundary);
    }

    #[test]
    fn meter_warns_before_and_errors_after_the_pacing_boundary() {
        assert_eq!(meter_color_band(2, 10, Some(40.0)), QuotaColorBand::OnPace);
        assert_eq!(
            meter_color_band(3, 10, Some(40.0)),
            QuotaColorBand::NearBoundary
        );
        assert_eq!(
            meter_color_band(4, 10, Some(40.0)),
            QuotaColorBand::NearBoundary
        );
        assert_eq!(
            meter_color_band(5, 10, Some(40.0)),
            QuotaColorBand::OverBoundary
        );
    }

    #[test]
    fn default_meter_bands_are_green_yellow_and_red() {
        let colors = ColorState::new(7);
        assert_eq!(
            quota_band_color(QuotaColorBand::OnPace, &colors),
            Color::AnsiValue(10)
        );
        assert_eq!(
            quota_band_color(QuotaColorBand::NearBoundary, &colors),
            Color::AnsiValue(11)
        );
        assert_eq!(
            quota_band_color(QuotaColorBand::OverBoundary, &colors),
            Color::AnsiValue(9)
        );
    }
}
