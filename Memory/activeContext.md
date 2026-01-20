---
version: "3.7"
lastUpdated: "2026-01-20"
lifecycle: core
stakeholder: pknull
changeTrigger: "session end, significant changes"
validatedBy: "session synthesis"
dependencies: ["projectbrief.md"]
---

# Active Context

## Current State

Project expanded with system monitors and utilities:
- Bonsai tree generator with static, live, infinite, and print modes
- 19 visualization algorithms implemented
- Interactive controls (speed, color schemes, pause)
- CLI via clap with comprehensive options
- **CPU monitor** with btop-style braille graphs and per-core display
- **Globe visualization** with rotation and color scheme support
- **Weather display** with wttr.in integration
- **Folding@home monitor** with real-time WebSocket updates
- **Audio visualizer** with stereo separation and decay animation
- **Help system**: `?` key shows contextual help overlay in all visualizers

## Recent Changes

### Session 2026-01-20 (Help System Implementation)

**Goal**: Add contextual help overlay (`?` shortcut) to all visualizers showing keyboard controls

**Accomplishments**:

1. **Core VizState changes** (`src/viz/mod.rs`):
   - Added `show_help` and `help_text` fields to `VizState` struct
   - Modified `VizState::new()` to accept `help_text: &'static str` parameter
   - Added `?` key handler to toggle help overlay
   - Implemented `render_help()` method with centered bordered box using Unicode box-drawing chars

2. **Global help section** (appended to all help overlays):
   ```
   ───────────────────────
    GLOBAL CONTROLS
    Space   Pause/resume
    1-9     Speed (1=fast)
    !-()    Color scheme
    q/Esc   Quit
    ?       Close help
   ───────────────────────
   ```

3. **Visualizers with custom help text** (6 files):
   - `globe.rs` - Pan (↑↓/jk), zoom (+/-), reset (0)
   - `invaders.rs` - Move (←→/hl), fire (Space), AI (A), reset (R)
   - `hypercube.rs` - Dimension (↑↓), zoom (←→/+/-)
   - `audio.rs` - Bar count (←→)
   - `lissajous.rs` - Cycle harmonics (H)

4. **Local implementations** (visualizers that don't use VizState, 2 files):
   - `pong.rs` - Full local help with P1/P2 controls, AI toggles, reset
   - `clock.rs` - Date toggle (D), time format (T), seconds (S), auto-cycle (A), anti-burn (C)

5. **Remaining visualizers** (global controls only, 12 files):
   - fire, pipes, plasma, donut, waves, life, hex, rain, dygma, matrix, cube, keyboard
   - All pass empty help text and call `render_help()` to show global section

**Key Learnings**:
- **Validated Pattern**: Shared `VizState` struct enables consistent behavior across visualizers
- **Validated Pattern**: Separating global vs visualizer-specific help keeps content manageable
- **Validated Pattern**: Box-drawing chars (`┌─┐│└─┘`) provide clean overlay borders
- **Technique**: Files with local implementations (pong, clock) require inline help rendering

**Files modified** (20 files):
- `src/viz/mod.rs` (core changes)
- 6 visualizers with custom help (globe, invaders, hypercube, audio, lissajous)
- 2 visualizers with local help (pong, clock)
- 12 visualizers with global-only help (fire, pipes, plasma, donut, waves, life, hex, rain, dygma, matrix, cube, keyboard)

### Session 2026-01-19 (Codex Code Review & Fixes)

**Goal**: Get external code review from Codex and implement all recommended fixes

**Accomplishments**:

1. **Codex code review obtained** covering:
   - Code quality and Rust idioms
   - Potential bugs and edge cases
   - Performance concerns
   - Architecture/design patterns

2. **8 fixes implemented**:

   | Fix | Description | Files |
   |-----|-------------|-------|
   | 1 | Rename `Box` → `Rect` to avoid shadowing `std::boxed::Box` | layout.rs, cpu.rs, mem.rs, disk.rs, diskio.rs, net.rs, gpu.rs |
   | 2 | Use `saturating_sub` to prevent width underflow panic | layout.rs:64 |
   | 3 | Replace `unwrap()` with `unwrap_or_default()` on SystemTime | fractal.rs:15 |
   | 4 | Remove nested `unwrap()` in timestamp conversion | sunlight.rs:103 |
   | 5 | Preallocate audio sample buffers (eliminate per-frame allocations) | audio.rs |
   | 6 | Use `BufWriter` for terminal output performance | terminal.rs:254-302 |
   | 7 | Split `FractalConfig` into common + per-viz `FractalKind` enum | config.rs, main.rs, fractal.rs, life.rs, globe.rs |
   | 8 | Deduplicate keybinding logic via `ColorState` delegation | viz/mod.rs + all 17 visualizers |

3. **Default color scheme set to mono/white (scheme 7)** across all visualizations

**Key Learnings**:
- **Validated Pattern**: External AI review (Codex) catches different issues than self-review
- **Validated Pattern**: `saturating_sub` is safer than subtraction for layout math
- **Validated Pattern**: Delegating to a single source (ColorState) prevents drift
- **Pitfall**: Renaming a struct used across modules requires updating all imports AND type usages

**Files modified** (28 files total):
- config.rs, main.rs, fractal.rs (FractalKind refactor)
- terminal.rs (BufWriter optimization)
- monitor/layout.rs, cpu.rs, mem.rs, disk.rs, diskio.rs, net.rs, gpu.rs (Box→Rect)
- viz/mod.rs (ColorState delegation)
- All 17 visualizers (color_scheme accessor update)

### Session 2026-01-17 (Sunlight Bar Display)

**Goal**: Change sunlight visualization from sine wave arc to horizontal bar

**Accomplishments**:
- Modified `src/viz/sunlight.rs` to render horizontal bar instead of sine curve
- Preserved color gradient (warm red at midnight ends, cool blue at noon center)
- Kept all markers (☀ sunrise, ☾ sunset, ● current time) on the bar
- Updated module docstring to reflect new display style

**Files modified**: src/viz/sunlight.rs

### Session 2026-01-17 (Audit Review Implementation)

**Goal**: Implement all recommendations from AUDIT-REVIEW.md code audit

**Accomplishments**:

1. **Error handling improvements**:
   - `settings.rs`: Config parsing errors now printed to stderr instead of silently using defaults
   - `bonsai.rs`: Fixed `unwrap()` on `duration_since(UNIX_EPOCH)` with fallback for broken clocks
   - `fah.rs`: `load_private_key()` now returns `Result<(), String>` with descriptive error messages surfaced to stderr

2. **Code quality**:
   - `fah.rs`: Replaced 35-line manual date calculation with one-line `chrono::Utc::now().format()` call
   - `fah.rs`: `debug_log!` macro now opt-in via `TERMART_DEBUG` env var (prevents disk filling)
   - `main.rs`: Added `#![warn(clippy::unwrap_used)]` to catch future unwrap additions

3. **Test infrastructure created**:
   - `tests/smoke_test.rs`: 3 smoke tests (help, version, invalid command)
   - All tests passing

4. **Documentation**:
   - `README.md`: Added security note about `chmod 600` for config file with credentials

**Key Learnings**:
- **Validated Pattern**: `#![warn(clippy::unwrap_used)]` catches unwraps without breaking existing code
- **Validated Pattern**: Return `Result<(), String>` for simple error surfacing without adding thiserror dependency
- **Pitfall**: Manual date calculations are error-prone; always use chrono if already a dependency

### Session 2026-01-16 (Comprehensive Code Review & Fix Pass)

**Goal**: Review all 19 visualizers for bugs and code quality, fix all identified issues

**Accomplishments**:

1. **Parallel code reviews executed** (19 code-reviewer agents):
   - Each visualizer reviewed by dedicated agent
   - Reviews covered: Security, Logic, Edge Cases, Style
   - Common patterns identified across files

2. **Critical bugs fixed across all visualizers**:
   - **Zero-size terminal panics**: Added guards in pipes.rs, rain.rs, life.rs, matrix.rs, plasma.rs, fire.rs, waves.rs
   - **Division by zero**: Fixed in plasma.rs, donut.rs, sunlight.rs
   - **Memory explosion**: Added 16D cap in hypercube.rs
   - **Mutex poisoning**: Added recovery in keyboard.rs, dygma.rs, audio.rs
   - **Correctness bugs**: Fixed globe.rs double to_radians, clock.rs DST, pong.rs asymmetric collision

3. **Code quality improvements**:
   - Extracted magic numbers to constants (rain.rs, hex.rs, pong.rs)
   - Created `spawn_pipe()` module-level function (pipes.rs)
   - Added `MonitorSourceGuard` RAII struct for cleanup (audio.rs)
   - Fixed empty chunk panic in stereo conversion (audio.rs)
   - Added `Winner` enum and symmetric collision zones (pong.rs)

4. **Verification**:
   - All fixes compile (`cargo check` passed)
   - All 19 tmux instances restarted with new code
   - Process restart verified via TTY mapping

**Key Learnings**:
- **Validated Pattern**: Parallel code review agents provide comprehensive coverage
- **Validated Pattern**: Zero-size guards should be added proactively to any terminal app
- **Validated Pattern**: RAII guards (like MonitorSourceGuard) safer than manual cleanup
- **Validated Pattern**: tmux pane restart via TTY→process mapping works reliably
- **Pitfall**: Fix agents may have Edit/Write auto-denied - apply fixes from main session

**Files modified** (19 visualizers):
- pipes.rs, rain.rs, hex.rs, pong.rs, audio.rs (direct fixes)
- life.rs, fire.rs, matrix.rs, plasma.rs, donut.rs, clock.rs (agent fixes)
- hypercube.rs, keyboard.rs, waves.rs, cube.rs (agent fixes)
- sunlight.rs, invaders.rs, globe.rs, dygma.rs (agent fixes)

### Session 2026-01-09/10 (Audio Visualizer Stereo + Decay Animation + Code Reviews)

**Goal**: Enhance audio visualizer with stereo separation, animated decay, and address all code review findings

**Accomplishments**:
- Stereo audio separation (left/right channels)
- 4-tier color decay animation
- Security fixes (TOCTOU race, command injection, log permissions)
- Code quality improvements (DebugLogger struct, extracted helpers)

### Session 2026-01-08 (N-Dimensional Hypercube + Audio Visualizer)

**Goal**: Create N-dimensional hypercube visualization and audio spectrum visualizer

**Accomplishments**:
- `termart hypercube` - 1D through 16D rotating hypercube with braille rendering
- `termart audio` - CAVA-style spectrum display with cpal + spectrum-analyzer
- Constants modules and helper extraction for both

### Session 2026-01-06 (FAH Machine ID Fix + Sunlight Enhancements)

- Fixed machine ID computation (SHA256(RSA_modulus_N) instead of SHA256(full_SPKI))
- Demo mode with f.lux-style phases
- Kelvin-based temperature control

### Session 2026-01-04 (Docker Sorting Feature)

- Sorting added: CPU% → MEM% → NAME cycles
- Pattern follows ps.rs implementation

## Next Steps

- [x] ~~**Audit follow-up**: Full `unwrap()` audit across codebase~~ (Codex review covered this)
- [x] ~~**Help system**: Add `?` shortcut to show keyboard controls~~ (implemented 2026-01-20)
- [ ] **Testing**: Add unit tests for config parsing, crypto error paths, network failures
- [ ] **Consider**: Apply same constants/helper extraction pattern to remaining viz files
- [ ] **Optional refactor**: Consolidate cube.rs into hypercube.rs (hypercube handles 3D now)
- [ ] **Optional refactor**: Extract `shortest_angular_delta()` helper (3 use cases)
- [ ] **Optional refactor**: Extract evdev reconnection logic into shared module
- [ ] **Dygma visualization - transparent key display**: Show "·" instead of default layer letter
- [ ] Docker: Add `--host` flag for easier remote Docker connection
- [ ] Space Invaders AI: Continue testing bullet avoidance
- [ ] Add more visualization types (snake, breakout, tetris?)
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect
- [ ] **Help system enhancement**: Add help to sunlight.rs (currently no VizState)

## Active Decisions

None pending.

## Blockers

None.
