---
version: "1.3"
lastUpdated: "2025-12-16"
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

- [ ] Space Invaders AI: Continue testing bullet avoidance (occasional hits reported)
- [ ] Add more visualization types
- [ ] Consider color scheme customization via config file
- [ ] Potential: sixel/kitty graphics protocol support for higher fidelity
- [ ] Documentation improvements
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect

## Active Decisions

None pending.

## Blockers

None.
