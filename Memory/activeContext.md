---
version: "2.0"
lastUpdated: "2026-08-01 19:40 UTC"
lifecycle: "active"
synthesizedFrom: "events"
---

# Active Context

## What Was Accomplished (2026-08-01 — Dynamic Claude quota graphs)

<!-- wwa-session: 019fbbff-9fc8-7112-95af-e5721499c77f -->

- Updated `src/viz/tokeneater.rs` to consume Anthropic's current dynamic `limits` list rather
  than relying solely upon fixed legacy fields.
- Every returned quota now receives a graph using the API-provided scoped display name. This
  exposed the account's Fable weekly quota whilst retaining 5-hour and general 7-day graphs.
- Preserved compatibility with legacy `five_hour`, `seven_day`, Sonnet, and Opus fields.
  Legacy model graphs render only when Anthropic returns an actual bucket, rather than showing
  misleading permanent 0% placeholders for `null` values.
- Added regression coverage for the current Fable response shape and legacy fallback behavior,
  documented dynamic quota rendering in `README.md`, installed the release binary, and verified
  the installed `termart claude-tokens` visual in a temporary tmux pane.
- Rechecked the live endpoint after implementation: it returned session, weekly-all, and
  Fable-scoped limits; both `seven_day_sonnet` and `seven_day_opus` remained `null`.

## Next Steps

- Source changes in `src/viz/tokeneater.rs` and `README.md` remain uncommitted; commit and push
  them only when explicitly requested.
- No functional follow-up is required. Future scoped model limits should appear automatically
  when Anthropic adds them to the `limits` response.

## Verification (green)

- `cargo test --all-targets`: 62/62 passed (57 unit + 5 integration).
- `cargo clippy --all-targets -- -D warnings`: clean.
- `git diff --check`: clean.
- `cargo install --path . --force`: release build installed successfully.
- Live installed-binary capture displayed 5-Hour, 7-Day, and Fable graphs.
