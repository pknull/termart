---
version: "5.1"
lastUpdated: "2026-03-02"
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
- 20 visualization algorithms implemented (including fractal)
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
- **Fractal visualizer**: Braille-rendered fractals with 5 types (Julia, Mandelbrot, Burning Ship, Tricorn, Phoenix)
- **GitHub CI/CD**: Automated builds on push/PR, release workflow creates multi-platform binaries on version tags

## Recent Changes

| Date | Goal | Outcome |
|------|------|---------|
| 2026-03-02 | Fractal visualizer + GitHub CI | 5 fractal types with zoom/pan, CI workflow, release workflow (v0.2.0) |
| 2026-02-17 | Matrix rain async speeds + glitch effects | Frame-skip variation, 2% glitch chance, 104-char katakana set |
| 2026-02-10b | Fix cover art color saturation | Luminance-to-scheme mapping, removed RGB approximation |
| 2026-02-10 | TUI cover quality + mplay deprecation | Half-block rendering, MPRIS shuffle/repeat/volume, mplay archived |
| 2026-01-20 | Help system implementation | `?` overlay in all 20 visualizers with global+custom sections |
| 2026-01-19 | Codex code review fixes | 8 fixes: Box→Rect, saturating_sub, BufWriter, ColorState delegation |
| 2026-01-17 | Sunlight bar display | Horizontal bar replaces sine arc, markers preserved |
| 2026-01-17 | Audit review implementation | Error handling, clippy::unwrap_used, smoke tests |
| 2026-01-16 | Comprehensive code review | 19 parallel reviews, zero-size guards, RAII cleanup |
| 2026-01-09/10 | Audio stereo + decay | Left/right channels, 4-tier color decay, security fixes |
| 2026-01-08 | Hypercube + audio visualizer | 1-16D hypercube, CAVA-style spectrum |
| 2026-01-06 | FAH machine ID fix | SHA256(RSA_modulus_N), demo mode, Kelvin control |
| 2026-01-04 | Docker sorting | CPU% → MEM% → NAME cycle |

## Synthesized Patterns

### Terminal Rendering

- **Half-block rendering** (`▀` + fg/bg) doubles vertical resolution with minimal complexity
- **Terminal chars are ~2:1** — use `art_w = 2 * art_h_cells` for visually square output
- **Named colors vs RGB**: `Color::DarkBlue` renders through terminal theme; `Color::Rgb` bypasses it. Use enum values for consistency.
- **Luminance banding** (5 levels) sufficient for small terminal art while maintaining scheme consistency
- **Box-drawing chars** (`┌─┐│└─┘`) provide clean overlay borders
- **Unicode symbols** (⇌ ↻) over emoji (🔀🔁) — emoji don't render in monospace fonts
- `Color::Reset` for background clears bg without affecting fg

### Animation & Effects

- **Frame-skip** (integer modulo) creates more obvious speed variation than floating-point multipliers
- **Research existing implementations** (cmatrix, neo) before designing effects
- **Julia morphing > Mandelbrot zoom**: Julia set with animated constant stays visually interesting; Mandelbrot zoom eventually hits blank/boring regions regardless of autopilot algorithm

### Safety & Robustness

- **Zero-size guards** should be added proactively to any terminal app
- **RAII guards** (like MonitorSourceGuard) safer than manual cleanup
- **`saturating_sub`** is safer than subtraction for layout math
- **`#![warn(clippy::unwrap_used)]`** catches unwraps without breaking existing code
- **Return `Result<(), String>`** for simple error surfacing without adding thiserror dependency

### Architecture

- **Shared `VizState` struct** enables consistent behavior across visualizers
- **Delegating to single source** (ColorState) prevents drift
- **Separating global vs visualizer-specific** (help text) keeps content manageable
- **External AI review** (Codex) catches different issues than self-review
- **Parallel code review agents** provide comprehensive coverage

### Pitfalls

- Chromium/Electron MPRIS backend doesn't expose Shuffle, LoopStatus, or real Volume
- Must build `--release` when binary symlink points to `target/release/`
- Renaming a struct across modules requires updating all imports AND type usages
- Manual date calculations are error-prone; always use chrono if already a dependency
- Fix agents may have Edit/Write auto-denied — apply fixes from main session

## Next Steps

- [ ] **Testing**: Add unit tests for config parsing, crypto error paths, network failures
- [ ] **Consider**: Apply same constants/helper extraction pattern to remaining viz files
- [ ] **Optional refactor**: Consolidate cube.rs into hypercube.rs (hypercube handles 3D now)
- [ ] **Optional refactor**: Extract `shortest_angular_delta()` helper (3 use cases)
- [ ] **Optional refactor**: Extract evdev reconnection logic into shared module
- [ ] **Dygma visualization - transparent key display**: Show "·" instead of default layer letter
- [ ] Docker: Add `--host` flag for easier remote Docker connection
- [ ] Space Invaders AI: Continue testing bullet avoidance
- [x] Add fractal visualizer (Julia, Mandelbrot, etc.)
- [ ] Add more visualization types (snake, breakout, tetris?)
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect
- [ ] **Help system enhancement**: Add help to sunlight.rs (currently no VizState)
- [ ] **TUI cover**: Consider terminal image protocol support (sixel/kitty) for higher fidelity
- [ ] **TUI control**: Consider combining tui-cover and tui-control into unified player view

## Active Decisions

None pending.

## Blockers

None.
