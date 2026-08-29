//! Shape of one disk panel entry: a name/total rule, an optional I/O sparkline,
//! and the Used meter the sparkline sits directly above.

use crate::colors::ColorState;
use crate::monitor::layout::{
    cpu_gradient_color_scheme, draw_history_graph_scheme, draw_meter_btop_scheme, format_bytes,
    header_color_scheme, muted_color_scheme, SampleHistory, MIN_GRAPHED_METER_WIDTH,
};
use crate::terminal::Terminal;

/// Columns an entry's rows are indented under its rule.
const ENTRY_INDENT: usize = 2;

/// Width of the "IO%" / "Used" label field.
const ENTRY_LABEL_W: usize = 6;

/// Width of the "NNN%" figure both of an entry's rows report.
const ENTRY_PCT_W: usize = 5;

/// A single blank column between two adjacent fields.
const FIELD_GAP: usize = 1;

/// Column an entry's I/O sparkline and its Used meter both start at. Sharing
/// this origin, and the span beyond it, is what puts the sparkline directly
/// above the meter it describes.
pub(super) const METER_X: usize = ENTRY_INDENT + ENTRY_LABEL_W + ENTRY_PCT_W + FIELD_GAP;

/// Suffix naming what the trailing figure on a Used row measures.
const FREE_SUFFIX: &str = " free";

/// Columns held for that trailing figure even when every entry needs fewer, so
/// the meters do not slide sideways as the figures tick. A typical
/// "NNN.NGiB free" is exactly this wide.
const FREE_FIELD_W: usize = 13;

/// Dashes a rule keeps between its name and its size, so a long name is
/// truncated instead of colliding with the figure.
const MIN_RULE_FILL: usize = 2;

/// Opening of a rule line, before the entry's name.
const RULE_HEAD: &str = "── ";

/// The name an entry is rendered under: the mount point's last component, or
/// "root" for `/`, which has none.
pub(super) fn entry_name(mount_point: &str) -> &str {
    mount_point
        .rsplit('/')
        .find(|component| !component.is_empty())
        .unwrap_or("root")
}

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
pub(super) struct Entry<'a> {
    pub name: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    /// Current utilization and its history, absent when nothing backs the
    /// entry in /proc/diskstats.
    pub io: Option<(f32, &'a SampleHistory)>,
}

impl Entry<'_> {
    pub fn percent(&self) -> f32 {
        usage_percent(self.used, self.available)
    }

    fn free_str(&self) -> String {
        format!("{}{}", format_bytes(self.available), FREE_SUFFIX)
    }
}

/// Column geometry every entry in one render shares.
#[derive(Clone, Copy)]
pub(super) struct EntryGeometry {
    pub width: usize,
    pub meter_w: usize,
    pub io_lines: bool,
}

/// Columns the trailing "N free" figures need, held at a floor so a figure
/// that shortens does not drag every meter with it.
fn free_field_width(entries: &[Entry]) -> usize {
    entries
        .iter()
        .map(|entry| entry.free_str().chars().count())
        .max()
        .unwrap_or(0)
        .max(FREE_FIELD_W)
}

/// Columns the sparkline and the meter span, once the label fields and the
/// trailing figure have taken theirs.
fn meter_width(width: usize, free_w: usize) -> usize {
    width.saturating_sub(METER_X + FIELD_GAP + free_w)
}

/// The label field an entry's IO and Used rows share.
fn row_label(label: &str) -> String {
    format!(
        "{:indent$}{:<field$}",
        "",
        label,
        indent = ENTRY_INDENT,
        field = ENTRY_LABEL_W
    )
}

/// The percentage field both rows share, closing with the blank column that
/// separates it from the sparkline or the meter.
fn row_percent(percent: f32) -> String {
    format!("{:>width$.0}% ", percent, width = ENTRY_PCT_W - 1)
}

/// Rows an entry costs: its rule, its Used meter, and an IO line when it has
/// one and the panel is still drawing them.
fn entry_rows(has_io: bool, io_lines: bool) -> usize {
    2 + usize::from(has_io && io_lines)
}

/// Rows `count` entries cost together, including the blank spacers between
/// them when the panel can still afford those.
fn panel_rows(has_io: &[bool], count: usize, spacers: bool, io_lines: bool) -> usize {
    let entries: usize = has_io
        .iter()
        .take(count)
        .map(|io| entry_rows(*io, io_lines))
        .sum();
    entries + if spacers { count.saturating_sub(1) } else { 0 }
}

/// How a row budget is spent: how many entries fit, whether they keep their
/// spacers and their IO lines, and whether a row was kept back to report the
/// ones left out.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PanelPlan {
    pub entries: usize,
    pub spacers: bool,
    pub io_lines: bool,
    pub affordance: bool,
    pub rows: usize,
}

/// Degrade a full panel to fit `budget` rows, in order: blank spacers first,
/// then the IO lines, then whole entries from the end. Dropping entries keeps
/// a row back for the "+N more" affordance, and an entry is only ever dropped
/// whole, so a rule is never left without the meter it introduces.
fn plan_panel(budget: usize, has_io: &[bool]) -> PanelPlan {
    for (spacers, io_lines) in [(true, true), (false, true), (false, false)] {
        let rows = panel_rows(has_io, has_io.len(), spacers, io_lines);
        if rows <= budget {
            return PanelPlan {
                entries: has_io.len(),
                spacers,
                io_lines,
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
        io_lines: false,
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
    let meter_w = meter_width(width, free_field_width(entries));
    // Too narrow for a sparkline worth reading: drop the graph whole rather
    // than stack a stub above the meter.
    let graphed = meter_w >= MIN_GRAPHED_METER_WIDTH;
    let has_io: Vec<bool> = entries
        .iter()
        .map(|entry| graphed && entry.io.is_some())
        .collect();

    let plan = plan_panel(budget, &has_io);
    let geom = EntryGeometry {
        width,
        meter_w,
        io_lines: plan.io_lines && graphed,
    };
    (plan, geom)
}

/// Draw an entry's rule: "── name " dash-filled to the right edge, closing on
/// the entry's total size.
fn draw_rule(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    entry: &Entry,
    colors: &ColorState,
) {
    let muted = muted_color_scheme(colors);
    let total = format_bytes(entry.total);
    let head_w = RULE_HEAD.chars().count();
    let tail_w = FIELD_GAP + total.chars().count();
    let name = truncate_chars(
        &entry.name,
        width.saturating_sub(head_w + FIELD_GAP + tail_w + MIN_RULE_FILL),
    );

    term.set_str(x, y, RULE_HEAD, Some(muted), false);
    let mut pos = x + head_w as i32;
    term.set_str(pos, y, &name, Some(header_color_scheme(colors)), false);
    pos += (name.chars().count() + FIELD_GAP) as i32;

    let fill = width.saturating_sub((pos - x) as usize + tail_w);
    for column in 0..fill {
        term.set(pos + column as i32, y, '─', Some(muted), false);
    }

    term.set_str(
        x + width as i32 - total.chars().count() as i32,
        y,
        &total,
        Some(muted),
        false,
    );
}

/// Draw the label and percentage an entry's IO and Used rows share, ending in
/// the column before the sparkline or the meter.
fn draw_row_head(
    term: &mut Terminal,
    x: i32,
    y: i32,
    label: &str,
    percent: f32,
    colors: &ColorState,
) {
    let head = row_label(label);
    term.set_str(x, y, &head, Some(muted_color_scheme(colors)), false);
    term.set_str(
        x + head.chars().count() as i32,
        y,
        &row_percent(percent),
        Some(cpu_gradient_color_scheme(percent, colors)),
        false,
    );
}

/// Draw one entry — its rule, its I/O sparkline when it has one and the panel
/// can still afford the row, and its Used meter — and return the row the next
/// entry starts on.
pub(super) fn draw_entry(
    term: &mut Terminal,
    x: i32,
    y: i32,
    entry: &Entry,
    geom: EntryGeometry,
    colors: &ColorState,
) -> i32 {
    draw_rule(term, x, y, geom.width, entry, colors);
    let mut cy = y + 1;

    if let Some((percent, history)) = entry.io.filter(|_| geom.io_lines) {
        draw_row_head(term, x, cy, "IO%", percent, colors);
        draw_history_graph_scheme(
            term,
            x + METER_X as i32,
            cy,
            geom.meter_w,
            history,
            cpu_gradient_color_scheme,
            colors,
        );
        cy += 1;
    }

    let percent = entry.percent();
    draw_row_head(term, x, cy, "Used", percent, colors);
    draw_meter_btop_scheme(term, x + METER_X as i32, cy, geom.meter_w, percent, colors);

    // The rule already carries the total and the meter carries the share used,
    // so used bytes would only restate them. What is left over is the figure an
    // operator acts on.
    let free = entry.free_str();
    term.set_str(
        x + geom.width as i32 - free.chars().count() as i32,
        cy,
        &free,
        Some(muted_color_scheme(colors)),
        false,
    );

    cy + 1
}

#[cfg(test)]
mod tests {
    use super::{
        entry_name, entry_rows, free_field_width, meter_width, panel_rows, plan_panel, row_label,
        row_percent, truncate_chars, usage_percent, Entry, PanelPlan, FREE_FIELD_W, METER_X,
    };

    fn entry(name: &str, available: u64) -> Entry<'static> {
        Entry {
            name: name.to_string(),
            total: available * 2,
            used: available,
            available,
            io: None,
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
    fn entry_names_are_the_mount_points_last_component() {
        assert_eq!(entry_name("/"), "root");
        assert_eq!(entry_name("/home"), "home");
        assert_eq!(entry_name("/boot/efi"), "efi");
        assert_eq!(entry_name("/mnt/rescue/"), "rescue");
        assert_eq!(entry_name(""), "root");
    }

    #[test]
    fn names_truncate_by_chars_not_bytes() {
        // Byte truncation would split a codepoint here and panic.
        assert_eq!(truncate_chars("ünïcødé-mount", 5), "ünïcø");
        assert_eq!(truncate_chars("efi", 8), "efi");
        assert_eq!(truncate_chars("efi", 0), "");
    }

    #[test]
    fn the_io_sparkline_and_the_used_meter_share_an_origin_and_a_width() {
        // Both rows are drawn from the same label and percentage fields, so
        // whatever follows them starts in the same column on each.
        assert_eq!(
            row_label("IO%").chars().count() + row_percent(31.0).chars().count(),
            METER_X
        );
        assert_eq!(
            row_label("Used").chars().count() + row_percent(47.0).chars().count(),
            METER_X
        );
        assert_eq!(
            format!("{}{}", row_label("Used"), row_percent(47.0)),
            "  Used    47% "
        );
        assert_eq!(
            format!("{}{}", row_label("IO%"), row_percent(100.0)),
            "  IO%    100% "
        );

        // And both span meter_width(), which leaves the trailing figure room.
        assert_eq!(meter_width(100, FREE_FIELD_W), 100 - METER_X - 1 - 13);
        assert_eq!(meter_width(20, FREE_FIELD_W), 0);
    }

    #[test]
    fn the_free_field_fits_the_widest_figure_and_never_shrinks_below_its_floor() {
        let narrow = [entry("root", 1024)];
        assert_eq!(free_field_width(&narrow), FREE_FIELD_W);

        let wide = [entry("root", 1024 * 1024 * 1024 * 1024 * 1023)];
        let widest = wide[0].free_str().chars().count();
        assert!(widest > FREE_FIELD_W);
        assert_eq!(free_field_width(&wide), widest);
    }

    /// Three entries, the middle one without an IO line: 8 rows of entries
    /// plus 2 spacers when the panel can afford everything.
    const MIXED: [bool; 3] = [true, false, true];

    #[test]
    fn compaction_drops_spacers_then_io_lines_before_any_entry() {
        let has_io = MIXED;

        assert_eq!(
            plan_panel(10, &has_io),
            PanelPlan {
                entries: 3,
                spacers: true,
                io_lines: true,
                affordance: false,
                rows: 10
            }
        );
        assert_eq!(
            plan_panel(9, &has_io),
            PanelPlan {
                entries: 3,
                spacers: false,
                io_lines: true,
                affordance: false,
                rows: 8
            }
        );
        assert_eq!(
            plan_panel(7, &has_io),
            PanelPlan {
                entries: 3,
                spacers: false,
                io_lines: false,
                affordance: false,
                rows: 6
            }
        );
    }

    #[test]
    fn compaction_drops_whole_entries_last_and_reports_the_ones_hidden() {
        let has_io = MIXED;

        // Only once spacers and IO lines are gone do entries go, and a row is
        // kept back for "+N more".
        assert_eq!(
            plan_panel(5, &has_io),
            PanelPlan {
                entries: 2,
                spacers: false,
                io_lines: false,
                affordance: true,
                rows: 5
            }
        );
        assert_eq!(
            plan_panel(1, &has_io),
            PanelPlan {
                entries: 0,
                spacers: false,
                io_lines: false,
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
                io_lines: false,
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
                let drawn = panel_rows(&has_io, plan.entries, plan.spacers, plan.io_lines);
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
    fn an_entry_costs_its_rule_its_meter_and_an_io_line_when_it_has_one() {
        assert_eq!(entry_rows(true, true), 3);
        assert_eq!(entry_rows(true, false), 2);
        assert_eq!(entry_rows(false, true), 2);
        assert_eq!(panel_rows(&[true, true], 2, true, true), 7);
        assert_eq!(panel_rows(&[true, true], 0, true, true), 0);
    }
}
