---
version: "2.0"
lastUpdated: "2026-07-29 04:31 UTC"
lifecycle: "active"
synthesizedFrom: "events"
---

# Active Context

## What Was Accomplished (2026-07-28 — Matrix repair, code audit, monitor controls)

<!-- wwa-session: 019faa14-96e8-7552-9732-d32ad5a249ad -->

- Repaired Matrix color switching across terminal protocols and environments:
  - Termart now emits functional visual colors even when a launcher exports `NO_COLOR`.
  - Shifted numeric shortcuts accept both symbol events (`!`) and CSI-u `Shift+1` events.
  - Matrix continues repainting color, help, and resize changes whilst paused.
  - Key-release events are ignored so enhanced-keyboard terminals do not double-toggle controls.
- Completed a broad correctness and safety review:
  - Corrected animation timing, Pomodoro elapsed-time accounting, CLI numeric bounds, tiny-pane
    arithmetic, fire resize handling, sunlight gamma restoration, FAH machine reconciliation,
    escaped mount paths, and cover-image resource limits.
  - Token usage requests no longer expose bearer credentials in process arguments; token files
    are written atomically with private permissions.
  - External text is stripped of terminal control characters before buffered rendering.
- Added a shared monitoring control model across CPU, memory, disk, I/O, network, GPU, process,
  and Docker monitors:
  - Persistent status rows report live/paused/error state, collection interval, color scheme,
    sample age, and brief action feedback.
  - `r` refreshes, `.` samples whilst paused, `+/-` adjusts the interval, and `d` restores the
    default. Input polling is independent of slow collection intervals.
  - Process and Docker rows support selection, stable identity across refresh/sort, scrolling,
    sorting, and detail overlays.
  - Collection failures remain visible and retry without terminating the interface.
- Bounded external Docker/NVIDIA collection:
  - Commands run in isolated process groups with a 3-second deadline.
  - Descendants are terminated, output capture is capped at 1 MiB, and displayed errors are
    bounded. Missing GPU backends are re-probed every five seconds.
- Three independent final review passes (correctness, edge cases, security) reported no remaining
  actionable findings. README and structured in-app help document the new controls.

## Next Steps

- Source changes remain uncommitted; commit/push them only when explicitly requested.
- Install the verified release binary and restart active dashboard panes when the Keeper wants
  the new Matrix and monitor controls in the live environment.
- Optional later work: persisted per-monitor preferences and short metric-history sparklines.

## Verification (green)

- `cargo test --all-targets`: 60/60 passed (55 unit + 5 integration).
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo build --release`: clean.
- `git diff --check`: clean.
- Live pseudo-TTY checks passed for Matrix color switching under `NO_COLOR`, paused repaint,
  slow-interval CPU controls, process selection/details, tiny panes, and bounded Docker/GPU exits.
- Descendant-held-pipe timeout and output-cap regression tests pass.
