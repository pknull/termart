---
version: "2.0"
lastUpdated: "2026-07-12 01:52 UTC"
lifecycle: "active"
synthesizedFrom: "events"
---

# Active Context

## What Was Accomplished (2026-07-11 — output consistency, monitor audit, Codex usage)

<!-- wwa-session: 019f52ae-ed96-70d3-9dfe-51b7a52dc7ef -->

- Replaced hand-formatted in-app help across all interactive commands with the reusable
  `HelpSpec`/`HelpEntry` schema in `src/help.rs`. Visualizers, monitors, games, and utilities
  now share alignment, separators, global-control profiles, modal rendering, and wording.
  Removed duplicated Clock and Pong overlay renderers and added CLI help coverage for every
  subcommand.
- Audited every system monitor for metric correctness, layout fit, and polling cost:
  - Network and Disk I/O now use logarithmic activity bars, actual elapsed sampling time,
    time-based peak decay, and precise byte-rate labels rather than arbitrary percentages.
  - Fixed Network's erroneous four-row minimum; its three-row form now fits the dashboard.
  - CPU preserves its header/footer by limiting core rows to available height.
  - Memory displays kernel `MemAvailable` and corrected cached percentages.
  - Disk capacity uses user-available blocks for `df`-like percentage semantics.
  - GPU caches AMD discovery, fixes NVIDIA detection/fan units, removes unused history, and
    corrects multi-GPU height calculation.
  - Process monitoring uses runtime clock-tick/page-size values and correctly filters kernel
    threads. Docker layout and polling floors were corrected.
- Added `termart codex-tokens` in `src/viz/codex_tokens.rs`. It reads the existing Codex CLI
  ChatGPT login from `~/.codex/auth.json`, fetches `/backend-api/wham/usage`, derives labels
  from server-provided window durations, shows reset countdowns/pacing/model-specific quotas,
  and applies exponential backoff. No separate credentials or guessed token limits are used.
- Extracted shared quota rendering to `src/viz/usage.rs`; Claude and Codex usage monitors now
  share duration, pacing, window-label, and bar presentation code.
- Added dirty-frame invalidation to both usage monitors. They continue polling input but only
  rebuild/present after API data, countdown-minute, resize, or UI changes; static widget state
  also removes meaningless pause/speed controls and enforces a 50 ms polling floor.
- Updated README coverage and globally installed the optimized binary. The active command
  `/home/pknull/bin/termart` resolves to the current `target/release/termart`.

## Next Steps

- Test `termart codex-tokens` inside the actual dashboard pane and tune its allocated height if
  model-specific quota rows should be visible alongside the 5-hour and 7-day windows.
- Visually confirm Network and Disk I/O logarithmic activity bars under both idle traffic and a
  sustained transfer; adjust scale floors/30-second half-life only from observed behavior.
- Optional dependency maintenance: `cargo install --locked` reports that transitive
  `core2 v0.4.0` is yanked; identify its dependency path before changing the lockfile.

## Verification (green)

- `cargo test --all-targets`: 24/24 passed (20 unit + 4 integration).
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo build --release`: clean.
- Live authenticated `codex-tokens` request and 60x12 pseudo-TTY render: no parse, auth, panic,
  or layout errors.
- `cargo install --path . --force --locked`: global install replaced successfully.
- `termart codex-tokens --help`: command available from the active global binary.
