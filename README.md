# termart

Terminal-based generative art, system monitors, and utilities.

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

### Visualizations
- **Bonsai**: Procedural ASCII bonsai trees with customizable growth
- **Matrix**: Classic cmatrix-style falling characters
- **Life**: Conway's Game of Life cellular automaton
- **Plasma**: Animated sine wave plasma effect
- **Fire**: Doom-style fire simulation
- **Rain**: Falling raindrops with splashes
- **Waves**: Animated ocean waves
- **Cube**: Rotating 3D wireframe cube
- **Pipes**: Classic pipes screensaver
- **Donut**: Rotating 3D torus (donut.c style)
- **Globe**: Rotating 3D globe with network nodes (eDEX-UI style)
- **Hex**: Hexagon grid with animated wave pulses
- **Keyboard**: Real-time keyboard visualization via evdev

### System Monitors
- **CPU**: Per-core usage with temperature and frequency
- **Memory**: RAM and swap usage with process breakdown
- **Disk**: Filesystem space usage
- **I/O**: Disk read/write rates
- **Network**: Interface traffic rates
- **GPU**: NVIDIA GPU stats (utilization, memory, temperature)

### Utilities
- **Clock**: Digital clock with nixie tube effects and date display
- **Weather**: Live weather display with ASCII art animations
- **Pomodoro**: Timer with ASCII tomato visualization
- **FAH**: Folding@home stats with real-time work unit progress

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

## Usage

### Bonsai Trees

```bash
termart bonsai                    # Generate a static tree
termart bonsai --live             # Live growth animation
termart bonsai --infinite         # Continuously generate trees
termart bonsai --print            # Print to stdout (no interactive display)
termart bonsai -L 50 -M 8         # Large bushy tree
termart bonsai -m "Hello!"        # Add message box
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-l, --live` | Show live growth animation | off |
| `-i, --infinite` | Keep generating trees | off |
| `-p, --print` | Print to stdout | off |
| `-t, --time <SEC>` | Animation step delay | 0.03 |
| `-w, --wait <SEC>` | Wait between trees (infinite mode) | 4.0 |
| `-L, --life <0-200>` | Initial branch life (higher = bigger) | 32 |
| `-M, --multiplier <0-20>` | Branch multiplier (higher = bushier) | 5 |
| `-s, --seed <NUM>` | Random seed for reproducibility | random |
| `-b, --base <0-2>` | Pot type: 0=none, 1=large, 2=small | 1 |
| `-c, --leaf <CHARS>` | Leaf characters (comma-separated) | & |
| `-m, --message <TEXT>` | Message to display | none |

### Visualizations

```bash
termart matrix                    # Matrix rain
termart life                      # Conway's Game of Life
termart plasma                    # Plasma effect
termart fire                      # Doom fire
termart rain                      # Rain animation
termart waves                     # Ocean waves
termart cube                      # 3D rotating cube
termart pipes                     # Pipes screensaver
termart donut                     # Rotating torus
termart globe                     # Globe with network nodes
termart hex                       # Hexagon grid
termart keyboard                  # Keyboard visualization
termart clock                     # Digital clock with nixie effects
```

**Common Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-t, --time <SEC>` | Animation speed (seconds per frame) | 0.03 |
| `-s, --seed <NUM>` | Random seed | random |
| `-d, --debug` | Show debug info | off |

**Life-specific:**
| Flag | Description | Default |
|------|-------------|---------|
| `-c, --char <CHAR>` | Character for cells | # |

### Interactive Controls (Visualizations)

| Key | Action |
|-----|--------|
| `1-9` | Change speed (1=fastest, 9=slowest) |
| `Shift+0-9` | Change color scheme |
| `Space` | Pause/Resume |
| `q` / `Esc` | Quit |

**Color Schemes:**
- `)` Shift+0: Green/Matrix
- `!` Shift+1: Fire (red/yellow)
- `@` Shift+2: Ice (blue/cyan)
- `#` Shift+3: Pink (magenta)
- `$` Shift+4: Gold (yellow/white)
- `%` Shift+5: Electric (cyan/white)
- `^` Shift+6: Lava (red/magenta)
- `&` Shift+7: Mono (grayscale)
- `*` Shift+8: Rainbow
- `(` Shift+9: Neon (blue/magenta)

### System Monitors

```bash
termart cpu                       # CPU usage per core
termart mem                       # Memory usage
termart disk                      # Disk space
termart io                        # Disk I/O rates
termart net                       # Network traffic
termart gpu                       # NVIDIA GPU stats
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-t, --time <SEC>` | Update interval | 1.0 |
| `-d, --debug` | Show debug info | off |

**Monitor Controls:**
| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |

### Clock

```bash
termart clock                     # Digital clock with nixie effects
termart clock --no-seconds        # Hide seconds display
```

**Features:**
- Alternates between time (8 seconds) and date (2 seconds)
- Nixie tube anti-poisoning effect (cycles through all digits)
- Blinking separators (colons/dashes)
- Unix timestamp display
- 12/24 hour format support
- Block-style digit display

**Display Format:**
- Time mode: `23:45:12` with `29-12-24 EST @1735519543` below
- Date mode: `29-12-24` with `23:45:12 EST @1735519543` below
- 12-hour mode shows AM/PM indicator

**Controls:**
| Key | Action |
|-----|--------|
| `C` | Trigger nixie tube cycling (all digits 0-9) |
| `+/-` | Adjust cycling speed |
| `D` | Toggle between date and time display |
| `S` | Toggle seconds display |
| `T` | Toggle 12/24 hour format |
| `A` | Toggle automatic date/time cycling |
| `1-9` | Change color schemes |
| `Space` | Pause/Resume |
| `q` / `Esc` | Quit |

**Nixie Tube Effects:**
- Digit transitions briefly show "8" (all segments lit)
- Anti-poisoning cycle runs every 5 minutes automatically
- Manual cycle with 'C' shows all digits counting 0-9 simultaneously

### Weather

```bash
termart weather                   # Auto-detect location via IP
termart weather -l "London"       # Specify city
termart weather -l "New York"     # City with spaces
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-l, --location <CITY>` | City name | auto-detect |
| `-t, --time <SEC>` | Animation speed | 0.1 |

**Features:**
- Animated ASCII art for weather conditions (sun, clouds, rain, snow, fog, storms)
- Day/night detection with appropriate visuals
- Temperature, humidity, wind speed, precipitation
- Auto-refreshes weather data periodically

**Controls:**
| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |

### Pomodoro Timer

```bash
termart pomodoro                  # Default: 25/5/15 minutes
termart pomodoro -w 50            # 50-minute work sessions
termart pomodoro -w 25 -s 5 -l 30 # Custom durations
termart pomodoro -c 6             # 6 pomodoros before long break
```

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `-w, --work <MIN>` | Work duration in minutes | 25 |
| `-s, --short-break <MIN>` | Short break duration | 5 |
| `-l, --long-break <MIN>` | Long break duration | 15 |
| `-c, --count <NUM>` | Pomodoros before long break | 4 |

**Features:**
- ASCII tomato fills as time progresses
- Big digit countdown display
- Progress bar and pomodoro dot tracker
- Terminal bell when timer ends
- Flashing display when awaiting input
- Color-coded phases: red (work), green (short break), blue (long break)

**Controls:**
| Key | Action |
|-----|--------|
| `Space` | Pause/Resume |
| `s` | Skip to next phase |
| `r` | Reset timer |
| `Enter` | Advance when timer done |
| `q` / `Esc` | Quit |

### Folding@home

```bash
termart fah                       # Display FAH stats and work units
```

**Features:**
- Real-time progress for local and remote machines
- Live WebSocket updates from FAH relay
- Account stats (points, WUs, rank)
- C/G indicator for CPU/GPU work units

**Configuration:**

Create `~/.config/termart/config.toml`:

```toml
[fah]
username = "your_username"
# Optional: for remote machine monitoring
fah_secret = "base64_private_key_from_browser"
fah_sid = "session_id_from_browser"
```

To get `fah_secret` and `fah_sid` for remote machines:
1. Log into https://v8-4.foldingathome.org
2. Open browser DevTools → Application → Local Storage
3. Copy `fah-secret` and `fah-sid` values

**Controls:**
| Key | Action |
|-----|--------|
| `r` | Refresh data |
| `q` / `Esc` | Quit |

## Notes

### Keyboard Visualization

Requires Linux evdev access to monitor global key events:

```bash
# Add yourself to the input group
sudo usermod -aG input $USER
# Log out and back in for changes to take effect
```

### GPU Monitor

Requires NVIDIA GPU with `nvidia-smi` available in PATH.

### Weather

Uses Open-Meteo API (free, no API key required). Location auto-detection uses ip-api.com.

## Examples

```bash
# Screensaver mode - infinite bonsai trees
termart bonsai -i -w 5

# Large tree with cherry blossom leaves
termart bonsai -L 60 -M 10 -c "❀,✿,❁"

# Birthday greeting
termart bonsai -m "Happy Birthday!" --live

# Quick system check
termart cpu -t 0.5

# Weather for Paris
termart weather -l "Paris"

# 50-minute focus sessions
termart pomodoro -w 50 -s 10

# Classic donut
termart donut

# eDEX-UI style globe
termart globe

# Live keyboard heatmap
termart keyboard

# Digital clock with nixie effects
termart clock

# Clock without seconds, 12-hour format
termart clock --no-seconds  # Then press 'T' for 12-hour
```

## License

MIT License - see [LICENSE](LICENSE) for details.
