---
version: "2.0"
lastUpdated: "2026-06-21 08:10 UTC"
lifecycle: "active"
synthesizedFrom: "events"
---

# Active Context

> Handoff note: synthesizer left the prior session's prompt list as the WWA
> (bg-session-save-handoff-gap — this background session delivered most edits via
> Bash cp from worktrees, so pattern_analyzer saw no project-dir Edit events).
> WWA + Next Steps below were written manually from the actual session work.

## What Was Accomplished (2026-06-21 — claude-tokens rate-limit + monitor bugfixes)

- Diagnosed the `claude-tokens` widget (`src/viz/tokeneater.rs`) rate-limit error:
  Anthropic now aggressively rate-limits the read-only `/api/oauth/usage` endpoint
  (429, no usable Retry-After; shared across all OAuth-token consumers), worsened
  locally by an expired termart stored token forcing per-cycle refreshes.
- **9380f29** fix(claude-tokens): fall back to Claude Code's `~/.claude/.credentials.json`
  when the stored-token refresh fails; detect real HTTP status via `curl -w`;
  exponential backoff (cap 30m) with on-screen "retry in Ns"; default poll 300→600s.
- **7ae27b7** chore: stopped tracking `.claude/` (git rm --cached settings.json, kept on
  disk) and ignored `.claude/`, `.codex`, `Work/`.
- Ran a multi-agent `/code-review` of the 47-file / 3,693-line uncommitted diff
  (6 finder agents). Verdict: ~95% cargo fmt + clippy idioms; one real new change in fah.
- **8e376d6** style: cargo fmt + clippy idioms across 43 files (no behavioral change).
- **2765d25** fix(fah): `fetch_remote_machines_with_sid` now returns a 3-state
  `SidFetch {Ok, AuthRejected, Transient}`; callers fall back to email/password
  (password-in-URL login) ONLY on AuthRejected (HTTP 401/403), not transient errors
  or empty accounts — stops repeated password exposure on a flaky SID.
- **f8422f1** fix(monitor): corrected ~2x net/disk I/O rate (prev_* counter lagged a
  cycle in net.rs/diskio.rs) and char-safe truncation in disk.rs/docker.rs (was
  byte-slicing → multibyte panic).
- **ab608c6** chore: committed floating Memory/ files.
- All 6 commits pushed to origin/master; working tree clean.

## Verification (green)

- `cargo build` clean; `cargo clippy` clean (no warnings); `cargo test` 16/16 pass
  (13 unit + 3 smoke). Gated before every commit. Debug binary current with HEAD.
- NOT built: optimized `--release` binary (only `target/debug/termart` exists).

## Next Steps

- Verify FAH actually returns 401/403 for a rejected `fah_sid`; if it uses a
  different status/body, broaden the `AuthRejected` match in
  `src/fah.rs::fetch_remote_machines_with_sid` accordingly.
- Optional: `cargo build --release` if a production binary is needed.
- Repo is fully merged to origin/master with a clean tree — no floating changes.
