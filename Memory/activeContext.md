---
version: "1.1"
lastUpdated: "2025-12-14"
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

### Session 2025-12-14
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

- [ ] Add more visualization types
- [ ] Consider color scheme customization via config file
- [ ] Potential: sixel/kitty graphics protocol support for higher fidelity
- [ ] Documentation improvements
- [ ] FAH: Consider auto-reconnect on WebSocket disconnect

## Active Decisions

None pending.

## Blockers

None.
