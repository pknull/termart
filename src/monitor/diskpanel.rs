//! Shape of one disk panel entry: a header line naming the mount, and the IO
//! and Used rows whose bars stand in one column beneath it.

use crate::colors::ColorState;
use crate::monitor::layout::{
    cpu_gradient_color_scheme, draw_meter_btop_scheme, format_bytes, header_color_scheme,
    muted_color_scheme,
};
use crate::terminal::Terminal;

/// Columns an entry's rows are indented under its header.
const ENTRY_INDENT: usize = 2;

/// Width of the "IO%" / "Used" label field.
const ENTRY_LABEL_W: usize = 6;

/// Width of the "NNN%" figure both of an entry's rows report, including the
/// blank column that separates it from the bar it follows.
const ENTRY_PCT_W: usize = 5;

/// A single blank column between two adjacent fields.
const FIELD_GAP: usize = 1;

/// Column an entry's IO bar and its Used bar both start at. Sharing this
/// origin, and the span beyond it, is what lets the two fills be read against
/// each other rather than each against itself.
const BAR_X: usize = ENTRY_INDENT + ENTRY_LABEL_W;

/// Columns a bar keeps before it is dropped whole. Below this a fill reports a
/// quarter or a third rather than a share, which is worse than no bar at all.
const MIN_BAR_W: usize = 8;

/// Columns the device field keeps before it is dropped whole. A device name is
/// distinguished by its tail, so a stub of its head names nothing.
const MIN_DEVICE_W: usize = 8;

/// Suffix naming what the trailing figure on a Used row measures.
const FREE_SUFFIX: &str = " free";

/// Columns held for that trailing figure even when every entry needs fewer, so
/// the bars do not slide sideways as the figures tick. A typical
/// "NNN.NGiB free" is exactly this wide.
const FREE_FIELD_W: usize = 13;

/// Truncate to `max` characters. Char-wise, so a multibyte name is shortened
/// rather than split part-way through a codepoint.
fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Share of usable capacity in use, where usable is what is written plus what
/// the user may still write. Reserved blocks are neither, so they belong in
/// neither term.
fn usage_percent(used: u64, available: u64) -> f32 {
    let usable = used.saturating_add(available);
    if usable > 0 {
        (used as f32 / usable as f32) * 100.0
    } else {
        0.0
    }
}

/// One rendered panel entry: a mount point, or the aggregate swap area.
pub(super) struct Entry {
    /// The mount point the entry reports on, or "swap" for the swap area.
    pub name: String,
    /// The device or file backing it, as the kernel names it.
    pub device: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    /// Current utilization, absent when nothing backs the entry in
    /// /proc/diskstats. Swap has none, so it renders a Used row alone.
    pub io: Option<f32>,
}

impl Entry {
    pub fn percent(&self) -> f32 {
        usage_percent(self.used, self.available)
    }

    fn free_str(&self) -> String {
        format!("{}{}", format_bytes(self.available), FREE_SUFFIX)
    }
}

/// Column geometry every entry in one render shares, which is what aligns the
/// bars across entries as well as within one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EntryGeometry {
    pub width: usize,
    pub bar_w: usize,
    /// Whether the rows can still afford their trailing figure.
    pub values: bool,
    pub io_rows: bool,
}

/// Columns the trailing "N free" figures need, held at a floor so a figure
/// that shortens does not drag every bar with it.
fn value_field_width(entries: &[Entry]) -> usize {
    entries
        .iter()
        .map(|entry| entry.free_str().chars().count())
        .max()
        .unwrap_or(0)
        .max(FREE_FIELD_W)
}

/// Columns the bars span, and whether the rows keep their trailing figure.
///
/// The figure is the optional field, so it is what gives way as the panel
/// narrows: these rows read as meters first and as numbers second. Narrower
/// still and the bar goes too, leaving a label and a percentage — which is
/// what a bar of two or three columns would have reported anyway.
fn bar_geometry(width: usize, value_w: usize) -> (usize, bool) {
    let fixed = BAR_X + ENTRY_PCT_W;
    let with_value = width.saturating_sub(fixed + FIELD_GAP + value_w);
    if with_value >= MIN_BAR_W {
        return (with_value, true);
    }

    let bare = width.saturating_sub(fixed);
    if bare >= MIN_BAR_W {
        (bare, false)
    } else {
        (0, false)
    }
}

/// The label field preceding a row's bar. Its width is what puts the IO bar and
/// the Used bar in the same column.
fn row_label(label: &str) -> String {
    format!(
        "{:indent$}{:<field$}",
        "",
        label,
        indent = ENTRY_INDENT,
        field = ENTRY_LABEL_W
    )
}

/// The percentage a row reports after its bar, opening with the blank column
/// that separates the two.
fn row_percent(percent: f32) -> String {
    format!(" {:>3.0}%", percent)
}

/// A header line's three fields, already fitted to the panel width. An empty
/// device or capacity is one the width could not afford.
#[derive(Debug, PartialEq, Eq)]
struct HeaderFields {
    mount: String,
    device: String,
    total: String,
}

/// Fit an entry's header to `width`. The fields give way in order of what they
/// tell an operator who is running out of columns: the device first, then the
/// capacity, and the mount point that names the entry is truncated only once
/// it has the line to itself.
fn header_fields(entry: &Entry, width: usize) -> HeaderFields {
    let total = format_bytes(entry.total);
    let before_total = width.saturating_sub(total.chars().count() + FIELD_GAP);
    if before_total == 0 {
        return HeaderFields {
            mount: truncate_chars(&entry.name, width),
            device: String::new(),
            total: String::new(),
        };
    }

    let device_room = before_total.saturating_sub(entry.name.chars().count() + FIELD_GAP);
    let device = if device_room >= MIN_DEVICE_W {
        truncate_chars(&entry.device, device_room)
    } else {
        String::new()
    };

    HeaderFields {
        mount: truncate_chars(&entry.name, before_total),
        device,
        total,
    }
}

/// Rows an entry costs: its header, its Used row, and an IO row when it has
/// one and the panel is still drawing them.
fn entry_rows(has_io: bool, io_rows: bool) -> usize {
    2 + usize::from(has_io && io_rows)
}

/// Rows `count` entries cost together, including the blank spacers between
/// them when the panel can still afford those.
fn panel_rows(has_io: &[bool], count: usize, spacers: bool, io_rows: bool) -> usize {
    let entries: usize = has_io
        .iter()
        .take(count)
        .map(|io| entry_rows(*io, io_rows))
        .sum();
    entries + if spacers { count.saturating_sub(1) } else { 0 }
}

/// How a row budget is spent: how many entries fit, whether they keep their
/// spacers and their IO rows, and whether a row was kept back to report the
/// ones left out.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PanelPlan {
    pub entries: usize,
    pub spacers: bool,
    pub io_rows: bool,
    pub affordance: bool,
    pub rows: usize,
}

/// Degrade a full panel to fit `budget` rows, in order: blank spacers first,
/// then the IO rows, then whole entries from the end. Dropping entries keeps a
/// row back for the "+N more" affordance, and an entry is only ever dropped
/// whole, so a header is never left without the rows it introduces.
fn plan_panel(budget: usize, has_io: &[bool]) -> PanelPlan {
    for (spacers, io_rows) in [(true, true), (false, true), (false, false)] {
        let rows = panel_rows(has_io, has_io.len(), spacers, io_rows);
        if rows <= budget {
            return PanelPlan {
                entries: has_io.len(),
                spacers,
                io_rows,
                affordance: false,
                rows,
            };
        }
    }

    // A budget of no rows at all reports nothing, not even that it is hiding
    // something: there is no row to say it on.
    let affordance = budget > 0;
    let entries = budget.saturating_sub(usize::from(affordance)) / entry_rows(false, false);
    PanelPlan {
        entries,
        spacers: false,
        io_rows: false,
        affordance,
        rows: entries * entry_rows(false, false) + usize::from(affordance),
    }
}

/// Work out how much of the panel fits in `budget` rows, and the column
/// geometry the entries that do fit will share.
pub(super) fn plan_render(
    entries: &[Entry],
    width: usize,
    budget: usize,
) -> (PanelPlan, EntryGeometry) {
    let (bar_w, values) = bar_geometry(width, value_field_width(entries));
    let has_io: Vec<bool> = entries.iter().map(|entry| entry.io.is_some()).collect();

    let plan = plan_panel(budget, &has_io);
    let geom = EntryGeometry {
        width,
        bar_w,
        values,
        io_rows: plan.io_rows,
    };
    (plan, geom)
}

/// Draw an entry's header: the mount point, the device behind it, and how much
/// that device holds, flush right.
fn draw_header(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    entry: &Entry,
    colors: &ColorState,
) {
    let muted = muted_color_scheme(colors);
    let fields = header_fields(entry, width);

    term.set_str(
        x,
        y,
        &fields.mount,
        Some(header_color_scheme(colors)),
        false,
    );
    if !fields.device.is_empty() {
        let at = fields.mount.chars().count() + FIELD_GAP;
        term.set_str(x + at as i32, y, &fields.device, Some(muted), false);
    }
    if !fields.total.is_empty() {
        let at = width - fields.total.chars().count();
        term.set_str(x + at as i32, y, &fields.total, Some(muted), false);
    }
}

/// Draw one metric row: its label, its bar, the percentage that bar reports,
/// and the figure the row closes on when it has one and the panel can afford
/// it. Every position comes from the shared geometry, so the IO row and the
/// Used row line up by construction.
#[allow(clippy::too_many_arguments)]
fn draw_row(
    term: &mut Terminal,
    x: i32,
    y: i32,
    geom: EntryGeometry,
    label: &str,
    percent: f32,
    value: Option<&str>,
    colors: &ColorState,
) {
    let muted = muted_color_scheme(colors);
    term.set_str(x, y, &row_label(label), Some(muted), false);
    draw_meter_btop_scheme(term, x + BAR_X as i32, y, geom.bar_w, percent, colors);
    term.set_str(
        x + (BAR_X + geom.bar_w) as i32,
        y,
        &row_percent(percent),
        Some(cpu_gradient_color_scheme(percent, colors)),
        false,
    );

    if let Some(value) = value.filter(|_| geom.values) {
        let at = geom.width.saturating_sub(value.chars().count());
        term.set_str(x + at as i32, y, value, Some(muted), false);
    }
}

/// Draw one entry — its header, its IO row when it has one and the panel can
/// still afford the row, and its Used row — and return the row the next entry
/// starts on.
pub(super) fn draw_entry(
    term: &mut Terminal,
    x: i32,
    y: i32,
    entry: &Entry,
    geom: EntryGeometry,
    colors: &ColorState,
) -> i32 {
    draw_header(term, x, y, geom.width, entry, colors);
    let mut cy = y + 1;

    // Utilization is a share of an interval, not of a capacity, so the row has
    // no byte figure to close on.
    if let Some(percent) = entry.io.filter(|_| geom.io_rows) {
        draw_row(term, x, cy, geom, "IO%", percent, None, colors);
        cy += 1;
    }

    // The header already carries the total and the bar carries the share used,
    // so used bytes would only restate them. What is left over is the figure an
    // operator acts on.
    let free = entry.free_str();
    draw_row(
        term,
        x,
        cy,
        geom,
        "Used",
        entry.percent(),
        Some(&free),
        colors,
    );

    cy + 1
}

#[cfg(test)]
mod tests {
    use super::{
        bar_geometry, entry_rows, header_fields, panel_rows, plan_panel, plan_render, row_label,
        row_percent, truncate_chars, usage_percent, value_field_width, Entry, EntryGeometry,
        HeaderFields, PanelPlan, BAR_X, ENTRY_PCT_W, FREE_FIELD_W, MIN_BAR_W,
    };

    fn entry(name: &str, available: u64) -> Entry {
        Entry {
            name: name.to_string(),
            device: "/dev/sdb2".to_string(),
            total: available * 2,
            used: available,
            available,
            io: None,
        }
    }

    /// A wide panel: everything fits, so nothing has given way.
    fn wide_geometry() -> EntryGeometry {
        let (bar_w, values) = bar_geometry(80, FREE_FIELD_W);
        EntryGeometry {
            width: 80,
            bar_w,
            values,
            io_rows: true,
        }
    }

    #[test]
    fn capacity_percentage_uses_user_available_space() {
        // 800 written, 100 writable and 100 reserved: the reserved blocks are
        // neither, so the row reads 800/900 rather than 800/1000.
        assert!((usage_percent(800, 100) - 88.888_89).abs() < 0.001);
        assert_eq!(usage_percent(0, 0), 0.0);
    }

    #[test]
    fn names_truncate_by_chars_not_bytes() {
        // Byte truncation would split a codepoint here and panic.
        assert_eq!(truncate_chars("ünïcødé-mount", 5), "ünïcø");
        assert_eq!(truncate_chars("efi", 8), "efi");
        assert_eq!(truncate_chars("efi", 0), "");
    }

    #[test]
    fn a_row_reads_label_then_bar_then_percent_then_an_optional_figure() {
        // The label field ends where the bar begins, and the percentage opens
        // with the column separating it from the bar's right edge.
        assert_eq!(row_label("Used"), "  Used  ");
        assert_eq!(row_percent(47.0), "  47%");
        assert_eq!(row_percent(100.0), " 100%");
        assert_eq!(row_percent(0.0), "   0%");
    }

    #[test]
    fn the_io_and_used_rows_share_a_bar_origin_and_a_width() {
        // Both rows are drawn from the same label field, so their bars start in
        // the same column whatever the label says.
        assert_eq!(row_label("IO%").chars().count(), BAR_X);
        assert_eq!(row_label("Used").chars().count(), BAR_X);

        // And the percentage that follows is a fixed field, so the figures line
        // up too however busy or full the entry is.
        for percent in [0.0, 5.0, 47.0, 99.4, 100.0] {
            assert_eq!(row_percent(percent).chars().count(), ENTRY_PCT_W);
        }

        // One geometry serves both rows and every entry, so the bars span the
        // same columns down the whole panel.
        let geom = wide_geometry();
        assert_eq!(geom.bar_w, 80 - BAR_X - ENTRY_PCT_W - 1 - FREE_FIELD_W);
        assert!(geom.values);
    }

    #[test]
    fn every_entry_in_one_render_is_given_the_same_geometry() {
        let entries = [entry("/", 1024), entry("/home", 900 << 30)];
        let (plan, geom) = plan_render(&entries, 80, 24);

        assert_eq!(plan.entries, 2);
        // The widest figure sets the field, so the narrow entry's bar ends
        // where the wide one's does rather than three columns further right.
        let (bar_w, values) = bar_geometry(80, value_field_width(&entries));
        assert_eq!(geom.bar_w, bar_w);
        assert!(values);
    }

    #[test]
    fn the_free_field_fits_the_widest_figure_and_never_shrinks_below_its_floor() {
        let narrow = [entry("/", 1024)];
        assert_eq!(value_field_width(&narrow), FREE_FIELD_W);

        let wide = [entry("/", 1024 * 1024 * 1024 * 1024 * 1023)];
        let widest = wide[0].free_str().chars().count();
        assert!(widest > FREE_FIELD_W);
        assert_eq!(value_field_width(&wide), widest);
    }

    #[test]
    fn a_narrowing_panel_drops_the_trailing_figure_before_the_bar() {
        // Wide enough for everything: the bar takes what the fixed fields and
        // the figure leave.
        let full = BAR_X + ENTRY_PCT_W + 1 + FREE_FIELD_W + MIN_BAR_W;
        assert_eq!(bar_geometry(full, FREE_FIELD_W), (MIN_BAR_W, true));

        // One column short of that, the figure goes rather than the bar, and
        // the bar takes the columns the figure was holding.
        assert_eq!(
            bar_geometry(full - 1, FREE_FIELD_W),
            (full - 1 - BAR_X - ENTRY_PCT_W, false)
        );
    }

    #[test]
    fn a_panel_too_narrow_for_a_meaningful_bar_still_reports_its_percentages() {
        // The narrowest width that keeps a bar at all, and the one below it,
        // where the row falls back to a label and a percentage.
        let bare = BAR_X + ENTRY_PCT_W + MIN_BAR_W;
        assert_eq!(bar_geometry(bare, FREE_FIELD_W), (MIN_BAR_W, false));
        assert_eq!(bar_geometry(bare - 1, FREE_FIELD_W), (0, false));
        assert_eq!(bar_geometry(0, FREE_FIELD_W), (0, false));

        // A bar of no columns draws nothing, so the percentage simply moves
        // left into the space it would have taken.
        let geom = EntryGeometry {
            width: 20,
            bar_w: 0,
            values: false,
            io_rows: true,
        };
        assert_eq!(BAR_X + geom.bar_w + ENTRY_PCT_W, 13);
    }

    #[test]
    fn a_header_carries_the_mount_point_its_device_and_its_capacity() {
        let mut disk = entry("/home", 512 * 1024 * 1024);
        disk.device = "/dev/nvme0n1p2".to_string();

        assert_eq!(
            header_fields(&disk, 60),
            HeaderFields {
                mount: "/home".to_string(),
                device: "/dev/nvme0n1p2".to_string(),
                total: "1.0GiB".to_string(),
            }
        );
    }

    #[test]
    fn a_narrow_header_sheds_fields_instead_of_colliding_them() {
        let mut disk = entry("/var/lib/containers", 512 * 1024 * 1024);
        disk.device = "/dev/mapper/vg-containers".to_string();

        // "/var/lib/containers" + gap + device + gap + "1.0GiB" is 52 columns.
        // A column short of that and the device is truncated, not overrun.
        let fitted = header_fields(&disk, 51);
        assert_eq!(fitted.mount, "/var/lib/containers");
        assert_eq!(fitted.device, "/dev/mapper/vg-container");
        assert_eq!(fitted.total, "1.0GiB");
        assert!(
            fitted.mount.chars().count() + 1 + fitted.device.chars().count()
                <= 51 - 1 - fitted.total.chars().count()
        );

        // Once what is left would only stub the device, it goes whole.
        let stubbed = header_fields(&disk, 34);
        assert_eq!(stubbed.mount, "/var/lib/containers");
        assert_eq!(stubbed.device, "");
        assert_eq!(stubbed.total, "1.0GiB");

        // And once the capacity no longer fits either, the mount point has the
        // line to itself and is truncated to it.
        let alone = header_fields(&disk, 6);
        assert_eq!(alone.mount, "/var/l");
        assert_eq!(alone.device, "");
        assert_eq!(alone.total, "");
    }

    /// Three entries, the middle one without an IO row: 8 rows of entries plus
    /// 2 spacers when the panel can afford everything.
    const MIXED: [bool; 3] = [true, false, true];

    #[test]
    fn compaction_drops_spacers_then_io_rows_before_any_entry() {
        let has_io = MIXED;

        assert_eq!(
            plan_panel(10, &has_io),
            PanelPlan {
                entries: 3,
                spacers: true,
                io_rows: true,
                affordance: false,
                rows: 10
            }
        );
        assert_eq!(
            plan_panel(9, &has_io),
            PanelPlan {
                entries: 3,
                spacers: false,
                io_rows: true,
                affordance: false,
                rows: 8
            }
        );
        assert_eq!(
            plan_panel(7, &has_io),
            PanelPlan {
                entries: 3,
                spacers: false,
                io_rows: false,
                affordance: false,
                rows: 6
            }
        );
    }

    #[test]
    fn compaction_drops_whole_entries_last_and_reports_the_ones_hidden() {
        let has_io = MIXED;

        // Only once spacers and IO rows are gone do entries go, and a row is
        // kept back for "+N more".
        assert_eq!(
            plan_panel(5, &has_io),
            PanelPlan {
                entries: 2,
                spacers: false,
                io_rows: false,
                affordance: true,
                rows: 5
            }
        );
        assert_eq!(
            plan_panel(1, &has_io),
            PanelPlan {
                entries: 0,
                spacers: false,
                io_rows: false,
                affordance: true,
                rows: 1
            }
        );
        // No rows at all: nothing is drawn, not even the affordance.
        assert_eq!(
            plan_panel(0, &has_io),
            PanelPlan {
                entries: 0,
                spacers: false,
                io_rows: false,
                affordance: false,
                rows: 0
            }
        );
    }

    #[test]
    fn a_plan_never_overflows_its_budget_or_splits_an_entry() {
        for has_io in [
            vec![],
            vec![true],
            vec![false, false],
            vec![true, false, true, true],
        ] {
            for budget in 0..24 {
                let plan = plan_panel(budget, &has_io);

                assert!(
                    plan.rows <= budget,
                    "{:?} overflows a budget of {}",
                    plan,
                    budget
                );
                assert!(plan.entries <= has_io.len());

                // Every planned row belongs to a whole entry, a spacer between
                // two of them, or the affordance for the ones left out.
                let drawn = panel_rows(&has_io, plan.entries, plan.spacers, plan.io_rows);
                assert_eq!(plan.rows, drawn + usize::from(plan.affordance));
                assert!(!plan.affordance || plan.entries < has_io.len());
                assert!(
                    plan.entries == has_io.len() || plan.affordance || budget == 0,
                    "hidden entries go unreported at a budget of {}",
                    budget
                );
            }
        }
    }

    #[test]
    fn an_entry_costs_a_header_a_used_row_and_an_io_row_when_it_has_one() {
        assert_eq!(entry_rows(true, true), 3);
        assert_eq!(entry_rows(true, false), 2);
        // An entry with no utilization to report — swap — costs two rows even
        // where the panel is drawing IO rows.
        assert_eq!(entry_rows(false, true), 2);
        assert_eq!(panel_rows(&[true, true], 2, true, true), 7);
        assert_eq!(panel_rows(&[true, true], 0, true, true), 0);
    }
}
