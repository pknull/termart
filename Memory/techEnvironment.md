---
version: "1.1"
lastUpdated: "2025-12-12"
lifecycle: core
stakeholder: pknull
changeTrigger: "tooling changes, dependency updates"
validatedBy: "build verification"
dependencies: []
---

# Tech Environment

## Language & Build

- **Language**: Rust (Edition 2021)
- **Toolchain**: Rust 1.86.0+
- **Package Manager**: Cargo

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| crossterm | 0.28 | Terminal manipulation, raw mode, events, colors |
| clap | 4.4 | CLI argument parsing (derive feature) |
| rand | 0.8 | Random number generation |
| libc | 0.2 | System calls (statvfs for disk stats) |
| evdev | 0.12 | Input device events |

## Build Commands

```bash
cargo build --release    # Optimized build (LTO enabled)
cargo install --path .   # Install to ~/.cargo/bin
cargo run -- bonsai      # Run bonsai mode
cargo run -- fire        # Run fire visualization
cargo run -- cpu         # Run CPU monitor
```

## Project Structure

```
src/
├── main.rs      # CLI entry, command dispatch
├── config.rs    # Configuration structs/enums
├── bonsai.rs    # Bonsai tree generation
├── fractal.rs   # Visualization algorithms
├── terminal.rs  # Terminal abstraction layer
└── monitor/     # System monitoring modules
    ├── mod.rs   # MonitorType, shared rendering helpers
    ├── cpu.rs   # CPU usage (/proc/stat)
    ├── mem.rs   # Memory usage (/proc/meminfo)
    ├── disk.rs  # Disk space (statvfs)
    ├── diskio.rs # Disk I/O rates (/proc/diskstats)
    ├── net.rs   # Network I/O (/proc/net/dev)
    └── gpu.rs   # GPU stats (nvidia-smi)
```

## Code Conventions

- snake_case for functions/variables
- PascalCase for types
- SCREAMING_CASE for constants
- Minimal comments (self-documenting preferred)
- Result-based error handling
- Buffer-based rendering pattern

## Key Patterns

- Stack-based iteration (avoid recursion for tree growth)
- Cellular automaton for Game of Life
- Particle systems for fire/rain
- 3D rotation matrices for cube/donut
- Double-buffering via Terminal struct

## CLI Structure

```
termart <COMMAND> [OPTIONS]

Visualizations:
  bonsai    # Tree generation (unique: -L life, -M multiplier, --live, --infinite, --print)
  matrix    # Matrix rain effect
  life      # Conway's Game of Life (unique: -c char)
  plasma    # Plasma effect
  fire      # Doom-style fire
  rain      # Rain animation
  waves     # Ocean waves
  cube      # Spinning 3D cube
  pipes     # Pipes screensaver
  donut     # Spinning donut (torus)
  globe     # Rotating globe
  hex       # Hexagonal grid
  keyboard  # Keyboard visualization

Monitors:
  cpu       # CPU usage per core
  mem       # RAM/swap usage
  disk      # Disk space usage
  io        # Disk I/O rates
  net       # Network I/O rates
  gpu       # GPU usage (NVIDIA via nvidia-smi)

Viz Options:
  -t, --time <TIME>   # Animation speed (default: 0.03)
  -s, --seed <SEED>   # Random seed
  -d, --debug         # Show debug info

Monitor Options:
  -t, --time <TIME>   # Update interval (default: 1.0)
  -d, --debug         # Show debug info
```

## Interactive Controls

| Key | Action |
|-----|--------|
| 1-9 | Speed (1=fastest) |
| 0 | Very slow |
| Shift+0-9 | Color schemes |
| Space | Pause/resume |
| q/Esc | Quit |
