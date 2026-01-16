---
version: "3.3"
lastUpdated: "2026-01-16"
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

## Recent Changes

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

- [ ] **Consider**: Apply same constants/helper extraction pattern to remaining viz files
- [ ] **Optional refactor**: Consolidate cube.rs into hypercube.rs (hypercube handles 3D now)
- [ ] **Optional refactor**: Extract `shortest_angular_delta()` helper (3 use cases)
- [ ] **Optional refactor**: Extract evdev reconnection logic into shared module
- [ ] **Dygma visualization - transparent key display**: Show "·" instead of default layer letter
- [ ] Docker: Add `--host` flag for easier remote Docker connection
- [ ] Space Invaders AI: Continue testing bullet avoidance
- [ ] Add more visualization types (snake, breakout, tetris?)
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect

## Active Decisions

None pending.

## Blockers

None.
