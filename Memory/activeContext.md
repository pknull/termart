---
version: "3.9"
lastUpdated: "2026-02-10"
lifecycle: core
stakeholder: pknull
changeTrigger: "session end, significant changes"
validatedBy: "session synthesis"
dependencies: ["projectbrief.md"]
---

# Active Context

## Current State

Project expanded with system monitors, utilities, and TUI media controls:
- Bonsai tree generator with static, live, infinite, and print modes
- 19 visualization algorithms implemented
- Interactive controls (speed, color schemes, pause)
- CLI via clap with comprehensive options
- **CPU monitor** with btop-style braille graphs and per-core display
- **Globe visualization** with rotation and color scheme support
- **Weather display** with wttr.in integration
- **Folding@home monitor** with real-time WebSocket updates
- **Audio visualizer** with stereo separation, decay animation, and width-fill bars
- **Help system**: `?` key shows contextual help overlay in all visualizers
- **TUI cover art**: Half-block rendering with bg color support, aspect-ratio preservation, luminance-to-scheme color mapping
- **TUI control**: Media controls with shuffle/repeat/volume, three-column status layout
- **mplay deprecated**: Archived on GitHub, termart covers core functionality

## Recent Changes

### Session 2026-02-10b (TUI Cover Palette Fix)

**Goal**: Fix scheme-colored cover art having colors too saturated compared to other termart elements

**Accomplishments**:

1. **Replaced RGB-approximation palette with luminance-to-scheme mapping** (`src/tui/cover.rs`):
   - Removed `expanded_palette()` (RGB interpolation between scheme colors) and `nearest_palette_color()` (Euclidean RGB matching)
   - Removed intermediate attempt: 256-entry luminance-gradient LUT with posterization — correct approach but still used `Color::Rgb` bypassing terminal palette
   - Final solution: `luminance_to_scheme()` converts pixel to luminance, maps to 5 bands (black + 4 scheme intensity levels), returns actual `Color` enum values from `scheme_color()`
   - Terminal renders these identically to audio bars, text, and all other scheme-colored elements
   - Removed ~100 lines of dead code: `color_to_rgb`, `ansi_value_to_rgb`, `ansi_256_to_rgb`, `rgb_6cube_value`, `lerp_u8`, `build_luminance_gradient`

**Key Learnings**:
- **Critical Pattern**: Terminal named colors (`Color::DarkBlue`) render through the terminal's palette/theme. `Color::Rgb { r: 0, g: 0, b: 255 }` bypasses the theme entirely. For visual consistency across UI elements, always use the same Color enum values.
- **Pitfall**: RGB approximations of terminal colors (DarkBlue→(0,0,128)) don't match what the terminal actually renders for `DarkBlue`. The terminal theme controls the actual color.
- **Validated**: Luminance banding (5 levels) provides sufficient detail for small terminal cover art while maintaining scheme consistency

**Files modified**: src/tui/cover.rs

### Session 2026-02-10 (TUI Cover Quality Overhaul + mplay Deprecation)

**Goal**: Improve tui-cover album art quality, bring tui-control to feature parity with mplay, deprecate mplay

**Accomplishments**:

1. **Half-block cover art rendering** (`src/terminal.rs`, `src/tui/cover.rs`, `src/viz/tui_cover.rs`):
   - Added `bg: Option<Color>` to Terminal Cell struct with `set_with_bg()` method
   - Updated `present()`, `render()`, `print_to_stdout()` with background color tracking
   - Replaced full-block (`█`) renderers with half-block (`▀`) using fg=top pixel, bg=bottom pixel
   - Doubles vertical resolution for cover art display
   - Switched `FilterType::Nearest` → `FilterType::Triangle` for smoother resize
   - Added `calc_cover_dimensions()` for aspect-ratio-preserving square layout (accounts for 2:1 terminal char ratio)
   - Scheme palette via luminance banding (see session 2026-02-10b for final approach)

2. **tui-control enhancements** (`src/viz/tui_control.rs`, `src/tui/mpris_client.rs`):
   - Fixed metadata order: title → artist → album (was title → album → artist)
   - Added volume to PlayerState, displayed on status line
   - Added shuffle/repeat state via MPRIS `get_shuffle()`/`get_loop_status()`
   - Three-column status layout: controls left (⇌ ⏸ ↻), time centered, volume right
   - Album shown in muted color to differentiate from title/artist

3. **Audio visualizer bar width control** (`src/viz/audio.rs`):
   - Changed left/right arrows from controlling bar count to controlling bar width
   - Bars always fill terminal width, count derived from `width / (bar_width + gap)`
   - Centered bar group to distribute leftover columns evenly

4. **mplay deprecation**:
   - Feature comparison confirmed tui-control covers ~75% of mplay features
   - Missing features (shuffle/repeat/volume) added to termart
   - Chromium/Electron MPRIS limitation identified (no shuffle/repeat/volume properties)
   - Archived `pknull/mplay` and `pknull/asha` repos on GitHub

**Key Learnings**:
- **Validated Pattern**: Half-block rendering (`▀` + fg/bg) doubles vertical resolution with minimal complexity
- **Validated Pattern**: Terminal chars are ~2:1, so `art_w = 2 * art_h_cells` for visually square output
- **Pitfall**: Emoji characters (🔀🔁🔂) don't render in terminal monospace fonts; use Unicode symbols (⇌ ↻)
- **Pitfall**: Chromium/Electron MPRIS backend doesn't expose Shuffle, LoopStatus, or real Volume
- **Pitfall**: Must build `--release` when binary symlink points to `target/release/`
- **Validated Pattern**: `Color::Reset` for background works in crossterm to clear bg without affecting fg

**Files modified**: terminal.rs, tui/cover.rs, viz/tui_cover.rs, tui/mpris_client.rs, viz/tui_control.rs, viz/audio.rs

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
- [ ] **TUI cover**: Consider terminal image protocol support (sixel/kitty) for higher fidelity
- [ ] **TUI control**: Consider combining tui-cover and tui-control into unified player view

## Active Decisions

None pending.

## Blockers

None.
