---
version: "3.0"
lastUpdated: "2026-01-08"
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
- Nine visualization algorithms implemented
- Interactive controls (speed, color schemes, pause)
- CLI via clap with comprehensive options
- **CPU monitor** with btop-style braille graphs and per-core display
- **Globe visualization** with rotation and color scheme support
- **Weather display** with wttr.in integration
- **Folding@home monitor** with real-time WebSocket updates

## Recent Changes

### Session 2026-01-08 (N-Dimensional Hypercube Visualization)
- **New visualization**: `termart hypercube` - N-dimensional rotating hypercube
  - Supports 1D through 16D (2^16 = 65,536 vertices max)
  - Uses braille characters for high-resolution rendering
  - Dynamic vertex/edge generation via bit manipulation
- **Key algorithms implemented**:
  - Vertex generation: Each vertex is combination of ±1 in each dimension (2^n total)
  - Edge generation: Connect vertices differing by exactly one coordinate (XOR single bit)
  - Multi-stage perspective projection: nD → 3D → 2D with clamped factors
  - Normalization: Project all vertices, find max extent, normalize for consistent scale
- **Controls**:
  - ↑/↓: Change dimension (1D-16D)
  - ←/→ or +/-: Zoom in/out (0.1x to 100x)
  - Space: Pause/resume
  - q: Quit
- **Iterative fixes through user feedback**:
  - Initial scaling inconsistent across dimensions → implemented max-extent normalization
  - High dimensions (9D+) collapsed due to compounded perspective → clamped factors to [0.2, 2.0]
  - Added manual zoom controls as fallback for edge cases
- **Code review and fixes**:
  - Added max_dim=16 to prevent memory exhaustion
  - Added max_zoom=100.0 to prevent overflow
  - Removed redundant `if d >= 1` condition (loop starts at d=3)
  - Updated doc comment from "3D-6D" to "1D through 16D"
- **Files modified**: `src/viz/hypercube.rs` (new), `src/config.rs`, `src/fractal.rs`, `src/main.rs`, `src/viz/mod.rs`

### Session 2026-01-06 (FAH Machine ID Fix)
- **Bug**: New machines (homebox) not appearing in FAH monitor
- **Root cause**: Machine ID computed from SHA256(full_SPKI_DER) instead of SHA256(RSA_modulus_N)
  - Same bug pattern as comparing `hash(serialized_object)` vs `hash(object.key_field)`
  - Account ID computation (working) used modulus N; machine ID (broken) used full SPKI bytes
- **Fix**: Parse SPKI DER → extract RSA public key → get modulus N → SHA256 → base64url
  - Added `RsaPublicKey` and `DecodePublicKey` imports
  - Machine ID computation now matches account ID pattern (lines 415-418 vs 680-687)
- **Hardcoded keys removed**: Dynamic key derivation works for all machines
  - Removed ~25 lines of temporary hardcoded AES session keys
  - All 3 machines (pk-lintop, PKWintop, homebox) now connect automatically
- **Security assessment**: Previously committed ephemeral AES keys assessed as no-risk
  - Keys are session-derived (rotate on reconnect) - those specific keys already stale
  - Require RSA private key (never committed) to decrypt new sessions
  - Panel consensus: No history rewrite needed
- **Sunlight tweak**: Demo mode displays HH:MM:00 instead of computed seconds

### Session 2026-01-06 (Sunlight Visualization Enhancements)
- **Demo mode added**: `--demo` flag cycles through day quickly
  - `--demo-speed` controls hours per second (default 2.0 = full day in 12 seconds)
  - Demo starts at current time, not midnight
  - Immediate gamma application on startup
- **f.lux-style phases implemented**:
  - Phase enum: Night, Sunrise, Day, Sunset
  - 1-hour smooth transitions using smoothstep easing (3t² - 2t³)
  - Phase displayed in colored text on visualization
- **Kelvin-based temperature control**:
  - `--night-temp` accepts Kelvin value (1900-6500)
  - Default 3400K matches f.lux default
  - Uses Tanner Helland algorithm (same as redshift/f.lux)
  - Individual `--night-blue` and `--night-green` still available for manual tuning
- **Critical xrandr fix**:
  - Bug: xrandr rejects gamma values of 0 ("gamma correction factors must be positive")
  - Low Kelvin values (1000K) produced blue=0.0
  - Fix: Clamp all gamma values to minimum 0.1 in both `kelvin_to_gamma()` and `temp_to_gamma()`
- **Public API**: `kelvin_to_gamma()` exported for CLI to convert --night-temp to gamma values

### Session 2026-01-04 (Docker Sorting Feature)
- **Sorting added to Docker monitor**:
  - Press `m` to cycle: CPU% → MEM% → NAME → CPU%
  - Header shows `[m]Sort:CPU%` indicator (or MEM%, NAME)
  - Default sort is CPU% descending
  - Hint line updated to include `m:Sort`
- **Pattern follows ps.rs implementation**:
  - `SortBy` enum with `next()` and `label()` methods
  - `cycle_sort()` and `sort_containers()` methods on DockerMonitor
  - Sorting applied after collecting containers in `update()`
- **Docker context usage**: Confirmed `docker context use remote` is preferred over DOCKER_HOST env var for users with existing context setup

### Session 2026-01-01 (Dygma LED Color Mapping Fix)
- **LED color display implemented and fixed**:
  - Initial implementation showed wrong colors for multiple keys (Z, \, Enter, ALT, CTL)
  - Root cause: Simple offset calculations don't work - layout has gaps and position swaps
  - Fix: Created `LED_MAP` lookup table from Bazecor source code (led_map in Keymap-ANSI.jsx)
- **Correct mapping chain discovered**:
  - Physical index → PHYSICAL_TO_KEYMAP → (row, col) → LED_MAP → LED index
  - PHYSICAL_TO_KEYMAP already handles quirks: ISO key gap, Enter/\ position swap
  - LED_MAP handles right side reverse ordering per row
- **LED_MAP structure** (from Bazecor):
  ```rust
  const LED_MAP: [[u8; 16]; 5] = [
      [0,1,2,3,4,5,6,255,255,39,38,37,36,35,34,33],     // Row 0
      [7,8,9,10,11,12,255,255,47,46,45,44,43,42,41,40], // Row 1
      [13,14,15,16,17,18,255,29,255,54,53,52,51,50,49,48], // Row 2
      [19,20,21,22,23,24,25,255,255,255,60,59,58,57,56,55], // Row 3
      [26,27,28,29,30,255,31,32,68,67,66,65,64,63,62,61],   // Row 4
  ];
  ```
  - Right side LEDs in **reverse order** per row (39,38,37... not 33,34,35...)
  - 255 = no LED at position
- **Key insight**: Enter is at keymap 31 (row 1), backslash at keymap 47 (row 2) - swapped from visual expectation
- **User verified**: Colors now display correctly

### Session 2026-01-01 (Globe Visualization Improvements)
- **Zoom formula refined** through iterative user feedback:
  - Initial: distance from user to farthest connection (too zoomed out)
  - Bounding box approach with lat/lon spans introduced
  - Final formula: `(1.0 / max_span).clamp(0.5, 2.5)` - conservative, shows all traffic
- **Twilight gradient** replaces binary day/night:
  - `daylight_level()` returns 0.0-1.0 with 18° (0.314 rad) twilight band
  - Continents render bright (>0.7) or dim based on gradient
  - Grid lines only on day side (>0.5)
- **City lights attempted then removed**:
  - Drew GLOBE_CITIES on night side when daylight_level < 0.3
  - Visual result: appeared as artifacts rather than city lights
  - Pattern reinforced: "sometimes removal is better than refinement"
- **Arc shortest path fixed**:
  - Bug: China traffic drew going west (long way) instead of east
  - Fix: Wrap longitude delta at ±π before interpolation
  - Same pattern as view_offset calculation (potential abstraction: `shortest_angular_delta()`)
- **Keyboard controls added**: ↑/↓ tilt, +/- zoom, 0 reset to auto-zoom

### Session 2026-01-01 (evdev Device Reconnection Fix)
- **Keyboard indicator fix**: Device occasionally stopped reporting keypresses
  - Root cause: evdev listener silently ignored ALL errors from `fetch_events()`
  - Including device disconnection - thread kept running but received no events
- **Fix applied to both keyboard and Dygma visualizations**:
  - `src/fractal.rs:run_keyboard()` - keyboard visualization
  - `src/viz/dygma.rs` - Dygma Raise visualization
- **Reconnection strategy**:
  - Store physical_path at thread start for device identification
  - Set non-blocking mode, filter EAGAIN/EWOULDBLOCK (normal for non-blocking)
  - Track consecutive real errors
  - After 50+ consecutive errors, trigger reconnection
  - Find device with same physical_path, or fallback to first keyboard
- **Borrow checker workaround**:
  - Problem: Can't reassign `device` in `Err` arm while `fetch_events()` borrows it
  - Solution: Use `need_reconnect` flag, handle reconnection at loop START (before borrow)
- **User verified**: Disconnect/reconnect test passed
- **Code duplication note**: Same ~40-line pattern now in two files (potential future refactor)

### Session 2025-12-29 (Clock Widget Feature Restoration)
- **Clock visualization fully restored**:
  - Anti-poisoning cycle on C key - all digits count 0-9 to exercise all segments
  - Transition effects restored - shows "8" briefly when digits change
  - Date/time alternation - automatically switches every 8 seconds
  - Full keyboard controls: D (toggle date/time), T (12/24hr), S (seconds), A (auto-cycle)
  - Full "88888888" display during date/time switches
- **Display improvements**:
  - Fixed separators: colon uses half-blocks (▄), dash uses top half-blocks (▀▀▀)
  - Added unix timestamp to date display
  - Changed to 2-digit year format (YY instead of YYYY)
  - Symmetrical info display - time shows date below, date shows time below
  - Original 3x3 half-block digit design preserved
- **Key learning**: When users want "minor tweaks", don't over-engineer - the original request was just to make digits "less blocky", not rebuild entire font system

### Session 2025-12-27 (Performance Optimizations)
- **Critical performance bug fixed in process monitor (ps.rs)**:
  - Was reading `/etc/passwd` for EVERY process to convert UID to username
  - This data was never displayed in the UI - pure dead code
  - Removed unused fields (state, mem_rss, uid, user) and functions
  - Impact: Eliminated thousands of file reads per update cycle
- **CPU monitor optimizations**:
  - Added caching for CPU model and shortened model string
  - CPU frequency updated every 10 cycles instead of every cycle
  - Thermal zone path discovered once at startup and cached
  - Impact: Reduced file I/O from every frame to periodic updates
- **Network/Disk monitors data structure optimization**:
  - Converted from HashMap<String, Stats> to Vec<Stats>
  - Eliminated string allocations and hash operations in hot paths
  - Better cache locality with contiguous memory
- **Key learnings**:
  - Dead code can cause severe performance issues
  - HashMap with String keys in render loops is expensive
  - Cache rarely-changing data (CPU model, thermal zones)
  - Vec with stable ordering often better than HashMap for small collections

### Session 2025-12-27 (Weather Widget Moon/Stars Fix)
- **Weather widget visual improvements**:
  - Improved star field: Fixed positions instead of random, gentle twinkling using ✦ and · characters
  - Multiple moon design iterations attempted (simplified ASCII, alignment fixes)
  - Ultimately removed moon entirely per user preference
- **Key learning**: ASCII art alignment is precise - single character offsets break visual cohesion
- **Pattern**: Sometimes removal is better than refinement when multiple iterations fail

### Session 2025-12-25 (Dygma Shift & Layer Fallback Fixes)
- **Layer 0 always included in fallback stack**:
  - Bug: Transparent keys on higher layers didn't fall through to base layer
  - Root cause: `if mask == 0 { mask = 1; }` only added layer 0 when NO layers active
  - Fix: Changed to `mask |= 1;` so layer 0 always available for transparent key fallback
  - Result: Numbers/symbols now correctly show shifted versions on all layers
- **Shift state fixes**:
  - Simplified shift tracking to direct press/release events
  - Shift clears on layer change (prevents stuck shift after layer switch)
  - Numbers and symbols now show !@#$%^&*() when shift held on any layer
- **Bazecor media keycodes added**:
  - 0x4CE2 (19682) = Mute, 0x5CE9 (23785) = Volume Up, 0x5CEA (23786) = Volume Down
  - 0x58B5-0x58CD = Next/Prev/Stop/Play, 0x5C6F-0x5C70 = Brightness
- **Debug output improved**:
  - Now shows `NumRow: 1:XXXX 2:XXXX ... shf:true/false` for number key diagnostics
- **Key learning**: Layer bitmask must always include layer 0 for proper transparent key handling

### Session 2025-12-25 (Dygma Layout Polish & UI Fixes)
- **Thumb cluster mappings finalized**:
  - Left thumb: T1→67 (BSP), T2→68 (SPC), T3→70 (DEL), T4→71 (>L3)
  - Right thumb: swapped T5/T7 positions (74, 75, 72, 73)
  - Position 69 is empty (0x0000) - not used
- **Layer key offsets corrected** (via debug output analysis):
  - ShiftToLayer: 0x4429 + layer (not 0x4400)
  - LockLayer: 0x4439 + layer
  - MoveToLayer: 0x4449 + layer
- **Right half alignment fixed**:
  - Rows 0, 2, 3 right-aligned (right edge matches)
  - Backspace now at position 7.0 (reaches edge)
- **Key widths improved**:
  - Tab/Caps/Shift widened to 1.5 units on left half
  - Letter keys shifted to accommodate (start at x=1.5)
- **Layout positioning**:
  - Left half aligned to left edge (x=1)
  - Right half aligned to right edge
  - Removed vertical scaling (eliminated row gaps)
- **UI polish**:
  - SuperKey labels simplified from "SKxx" to "SK"
  - Layer indicator moved to top with connection status: `[ Layer 1 : Focus ]`
  - Layers display 1-indexed (matching Bazecor UI)
- **Key learning**: Debug output (`LThumb: 67:002A 68:002C...`) invaluable for mapping verification

### Session 2025-12-24 (Dygma Layout Fix - Bazecor Source Analysis)
- **Bazecor source analysis**: Used official Keymap-ANSI.jsx from Dygma repository
  - Keymap uses 16-column grid: `keyIndex = row * 16 + col`
  - Identified gap positions: 7-8, 22-23, 38-40, 48 (ISO), 55-57, 71
  - Total: 68 physical keys matching JSX layout
- **Critical layout corrections**:
  - Right Row 1: Y U I O P [ ] Enter (Enter on right edge at keymap 31)
  - Right Row 2: H J K L ; ' \ (backslash at end, keymap 47)
  - Left Shift: keymap position 49 (ANSI doesn't use 48 - that's ISO key)
- **PHYSICAL_TO_KEYMAP rewritten**:
  - Left half: 0-6, 16-21, 32-37, 49-54, 64-66, 67-70 (thumb)
  - Right half: 9-15, 24-31, 41-47, 58-63, 76-79, 72-75 (thumb)
- **Previous fixes retained**:
  - LockLayer range: 0x4420-0x4440
  - MoveToLayer range: 0x4440-0x4460
  - F13-F24 keys (0x68-0x73), SuperKey support (0xD000-0xDFFF)

### Session 2025-12-23 (Dygma Keyboard Visualization - WIP)
- **New visualization**: `termart dygma` for Dygma Raise split keyboard
  - Focus protocol connection via serial (115200 baud, auto-detect by USB VID/PID)
  - Layer detection via `layer.state` command - updates in real-time
  - Keymap query via `keymap.custom` - loads all 9-10 layers × 80 keys
  - Physical layout with split halves, thumb clusters, columnar stagger
  - Keycode-to-label conversion for Kaleidoscope firmware codes
  - Shift state tracking via evdev - shows shifted chars (!@#$ etc)
  - Heat map from evdev key events (same as existing keyboard viz)
- **Technical details**:
  - Added `serialport = "4.3"` dependency
  - Created `src/viz/dygma.rs` (~1050 lines)
  - Key mapping based on official RaiseANSIKeyMap.png from Dygma firmware repo

### Session 2025-12-18 (Optimizations & Bug Fixes)
- **Docker monitor bug fix**: Fixed template parsing error with SSH Docker
  - `.Status` not valid for `docker stats` (only for `docker ps`)
  - Removed status field, updated parser to expect 5 fields
  - Changed row coloring to CPU gradient instead of status-based
- **Visualization optimizations** (fractal.rs):
  - **Globe**: Made continent/city data static with `LazyLock`
    - `GLOBE_CONTINENTS`: 10 continent outlines (238 points)
    - `GLOBE_CITIES`: 48 major world cities
    - Removed ~180 lines of per-call initialization
  - **Keyboard**: Made layout rows `const` slices
    - 6 `KB_ROW_*` consts for each keyboard row
    - Runtime builds `Vec<&[...]>` from const refs
  - **Rain**: Replaced Vec allocation pattern with `retain_mut`
    - Drops and splashes now filtered in-place
    - Made `SPLASH_CHARS` a const array
- **Affected visualizations**: `globe`, `keyboard`, `rain`

### Session 2025-12-18 (Glances-Inspired Monitors)
- **Process list** (`termart ps`): Top processes by CPU/MEM usage
  - Reads /proc/[pid]/stat for CPU ticks, RSS, state
  - Delta-based CPU% calculation (stores raw ticks between updates)
  - Press `m` to toggle sort between CPU% and MEM%
  - Simplified display: PID, CPU%, MEM%, PROCESS
- **Docker monitor** (`termart docker`): Container resource usage
  - Uses `docker stats --no-stream --format` (matches gpu.rs pattern)
  - Shows NAME, CPU%, MEM, MEM%, NET I/O, STATUS
  - Color-coded by status: green=running, yellow=paused, red=exited
  - Supports remote Docker via DOCKER_HOST env var
- **IO Wait metric**: Added to CPU monitor
  - Displays after uptime: `up 2d 05:32  IO Wait: 0.1%`
  - Color: muted <5%, yellow 5-20%, red >20%
  - Data was already parsed in CpuTimes, just wasn't displayed
- **Bug fixes**:
  - ps.rs: Fixed 625000% CPU bug (was storing cpu_pct instead of cpu_ticks)
  - ps.rs: Fixed column alignment with consistent double-space separators
  - ps.rs: Made `m` key re-sort immediately instead of waiting for next update
  - cpu.rs: Fixed IO Wait overlapping Load AVG on narrow terminals

### Session 2025-12-17 (Visualizations & Optimizations)
- **Clock widget**: 24-hour time in block letters with date and timezone
  - OnceLock for timezone caching (computed once)
  - Reusable string buffers, cached layout values
- **Pong game**: Two-player classic with AI toggle
  - W/S for P1, Up/Down for P2, 1/2 to toggle AI
  - Spin factor on paddle hits, win at 11 points
- **Optimization pass** across viz files:
  - clock.rs: OnceLock timezone, cached colors on scheme change
  - invaders.rs: Named constants, static UI strings, reusable buffers
  - matrix.rs: Fixed-size [char; 25] array instead of Vec
  - fractal.rs: SPEED_TABLE const, inlined hot functions, unrolled neighbor lookup
- **Plasma seed fix**: Was deterministic regardless of seed
  - Now randomizes: wave frequencies (4), phase offsets (4), radial center, time multipliers (3)
  - Same seed = same pattern, different seed = different pattern

### Session 2025-12-17 (Network Monitor Centering)
- **Horizontal centering**: Added max 80-char content width, centered in terminal
- **Vertical centering fix**: Removed `.max(0)` so content clips equally from top/bottom
- **Panel height fix**: Each interface uses 3 lines (name+download+upload), not 2
- **Key Learning**: Panel height calculations must match actual rendered line count

### Session 2025-12-16 (Submodule Rename)
- **Asha → asha**: Renamed submodule directory to lowercase
  - Updated .gitmodules, .gitignore, CLAUDE.md
  - Updated git config submodule section
  - Renamed .git/modules/Asha → .git/modules/asha
  - Updated worktree path in modules config

### Session 2025-12-16 (Space Invaders AI)
- **AI Bullet Avoidance**: Rewrote from zone-counting to predictive danger zones
  - Tracks where each bullet will land (impact_x = bullet.x since they fall straight)
  - Urgency-based safety margins: close bullets get 3.5px, distant get 2.5px
  - Dodge jumps 6 pixels to escape danger zones
- **Directional Intercept**: Only pursue aliens coming TOWARD player
  - Alien moving away → ignore, wait for it to return
  - Alien moving toward → calculate intercept point and move there
- **Proactive Positioning**: When no aliens approaching, move ahead to leading edge
  - Positions 5px ahead of where aliens will turn around at edge
  - Ready to shoot when they reverse direction
- **Scaled Alien Grid**: Columns scale to terminal width (41% proportion)
  - Matches original 11 cols on 80-char terminal
  - Min 5, max 30 columns for different terminal sizes
- **Key Learning**: "Chasing" vs "intercepting" distinction - never pursue moving-away targets

### Session 2025-12-14 (Bug Fix)
- **CPU Monitor Fix**: Fixed all monitors showing ~98% usage on startup
  - Root cause: First and second samples taken microseconds apart
  - With tiny time delta, any CPU activity appeared as near-100% usage
  - Fix: Added 100ms sleep between first update and main loop
  - Applied to all monitors: cpu, mem, gpu, diskio, net
- **Network Scaling**: Confirmed network monitor uses auto-scaling with peak tracking
  - 100% = observed peak rate (with slow decay toward 1 MB/s minimum)

### Session 2025-12-14 (FAH)
- **FAH Monitor Complete**: Real-time Folding@home display with WebSocket connection
  - AES-256-CBC encrypted message decryption
  - Local and remote machine progress tracking
  - Account stats (points, WUs, rank) from FAH API
  - Consistent btop-style progress bars with C/G CPU/GPU indicators
  - Config file support (~/.config/termart/config.toml)
- **Submodule Rename**: Renamed `asha` → `Asha` for consistency
  - Updated .gitmodules, .gitignore, CLAUDE.md, git config

### Previous Sessions
- Initial project creation (2025-12-09)
- ASHA framework integration
- Color scheme support added to all monitors

## Next Steps

- [ ] **Optional refactor**: Consider consolidating cube.rs into hypercube.rs
  - hypercube.rs now handles 3D (same as cube.rs)
  - Could remove separate `cube` command or make it alias to `hypercube` with dim=3
- [ ] **Optional refactor**: Extract `shortest_angular_delta()` helper
  - Same wrap-at-±π pattern used in: arc rendering, view_offset, farthest point detection
  - Three use cases now - worth abstracting
- [ ] **Optional refactor**: Extract evdev reconnection logic into shared module
  - Currently duplicated in `src/fractal.rs` and `src/viz/dygma.rs` (~40 lines each)
  - Low priority - only 2 use cases, may be premature abstraction
- [ ] **String allocation optimization**: Reduce allocations using `write!` instead of `format!` in render hot paths
- [ ] **Performance profiling**: Measure actual CPU usage improvements in monitors
- [x] **Dygma visualization - LED color display**: ✅ COMPLETED
  - Queries `palette` and `colormap.map` via Focus protocol
  - Uses LED_MAP lookup table from Bazecor for correct physical→LED mapping
  - Displays actual keyboard LED colors on keys
- [ ] **Dygma visualization - transparent/no-key display**:
  - Currently shows default layer letter for "none"/transparent keys (confusing)
  - Should show something like "·" or empty or "T" for transparent
  - Need to detect keycode 0x0000 or 0xFFFF (whichever means transparent)
- [ ] **Dygma visualization**: Continue testing with real keyboard
  - Most mappings verified working
  - Watch for any additional Kaleidoscope keycodes still showing as hex
- [ ] Docker: Add `--host` flag for easier remote Docker connection
- [ ] Docker: Enable remote Docker API on 172.16.0.14 (ports 2375/2376 not open)
- [ ] Space Invaders AI: Continue testing bullet avoidance (occasional hits reported)
- [ ] Add more visualization types (snake, breakout, tetris?)
- [ ] Consider color scheme customization via config file
- [ ] Potential: sixel/kitty graphics protocol support for higher fidelity
- [ ] Documentation improvements
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect
- [ ] Waves: Consider adding seed support like plasma

## Active Decisions

None pending.

## Blockers

None.
