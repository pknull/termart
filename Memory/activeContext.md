---
version: "1.8"
lastUpdated: "2025-12-18"
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
  - Created `src/viz/dygma.rs` (~970 lines)
  - Key mapping based on official RaiseANSIKeyMap.png from Dygma firmware repo
- **Known issues** (to revisit):
  - Physical layout geometry feels off
  - Some Kaleidoscope keycodes still show as hex
  - Thumb keys only show 2 of 4 (Dygma image shows 70-71, 72-73)
  - Overall feel is "clunky" - may need different approach

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

- [ ] **Dygma visualization**: Revisit and refine (see notes below)
  - Physical layout geometry feels "clunky" - needs refinement
  - Some keycodes still showing as hex - need more Kaleidoscope mappings
  - Index mapping between physical layout and keymap may have errors
  - "Something missing" - possibly needs a different approach entirely
  - Current state: Working layer detection, keymap query, shift tracking, but rough
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
