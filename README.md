# termart

Terminal-based generative art: bonsai trees and animated visualizations.

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Bonsai Trees**: Generate procedural ASCII bonsai trees with customizable parameters
- **Matrix Rain**: Classic cmatrix-style falling characters
- **Game of Life**: Conway's cellular automaton
- **Plasma**: Animated sine wave plasma effect
- **Fire**: Doom-style fire simulation
- **Rain**: Falling raindrops with splashes
- **Waves**: Animated ocean waves
- **3D Cube**: Rotating wireframe cube using braille characters
- **Pipes**: Classic pipes screensaver
- **Donut**: Rotating 3D torus (donut.c style)
- **Globe**: Rotating 3D globe with network activity nodes (eDEX-UI style), auto-detects user location
- **Hex Grid**: Hexagon grid with animated wave/pulse effects (eDEX-UI style)
- **Keyboard**: On-screen keyboard visualization with real-time global key monitoring via evdev

All visualizations support:
- Interactive speed control (1-9 keys)
- Multiple color schemes (Shift+1-9)
- Pause/resume (Space)
- Terminal resize handling

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
# Generate a static bonsai tree
termart bonsai

# Live growth animation
termart bonsai --live

# Infinite mode - continuously generate new trees
termart bonsai --infinite

# Print to stdout (for piping/screenshots)
termart bonsai --print

# Customize tree parameters
termart bonsai --life 50 --multiplier 8 --seed 12345

# Add a message box
termart bonsai --message "Happy Birthday!"
```

**Bonsai Options:**
- `-l, --live`: Show live growth animation
- `-i, --infinite`: Keep generating trees
- `-p, --print`: Print to stdout
- `-t, --time <SECONDS>`: Animation step delay (default: 0.03)
- `-w, --wait <SECONDS>`: Wait between trees in infinite mode (default: 4.0)
- `-L, --life <0-200>`: Initial branch life, higher = bigger tree (default: 32)
- `-M, --multiplier <0-20>`: Branch multiplier, higher = bushier (default: 5)
- `-s, --seed <NUMBER>`: Random seed for reproducibility
- `-b, --base <0-2>`: Pot type: 0=none, 1=large, 2=small (default: 1)
- `-c, --leaf <CHARS>`: Leaf characters, comma-separated (default: &)
- `-m, --message <TEXT>`: Message to display next to tree

### Visualizations

```bash
# Matrix rain (default)
termart viz

# Specify visualization type
termart viz -T matrix    # Classic falling characters
termart viz -T life      # Conway's Game of Life
termart viz -T plasma    # Sine wave plasma effect
termart viz -T fire      # Doom-style fire simulation
termart viz -T rain      # Raindrops with splashes
termart viz -T waves     # Ocean wave animation
termart viz -T cube      # Rotating 3D wireframe cube
termart viz -T pipes     # Classic pipes screensaver
termart viz -T donut     # Rotating 3D torus
termart viz -T globe     # Rotating globe with network nodes
termart viz -T hex       # Hexagon grid with wave pulses
termart viz -T keyboard  # On-screen keyboard (requires evdev access)

# Keyboard with debug info (shows F-keys row)
termart viz -T keyboard --debug
```

**Visualization Options:**
- `-T, --viz-type <TYPE>`: Visualization type (default: matrix)
- `-t, --time <SECONDS>`: Animation speed (default: 0.03)
- `-s, --seed <NUMBER>`: Random seed
- `-c, --char <CHAR>`: Character for drawing (life mode)
- `-d, --debug`: Show debug info (keyboard: displays F-keys and status)

### Interactive Controls

While running any visualization:
- `1-9`: Change speed (1=fastest, 9=slowest)
- `Shift+1-9`: Change color scheme
  - `)` (Shift+0): Green/Matrix
  - `!` (Shift+1): Fire (red/yellow)
  - `@` (Shift+2): Ice (blue/cyan)
  - `#` (Shift+3): Pink (magenta)
  - `$` (Shift+4): Gold (yellow/white)
  - `%` (Shift+5): Electric (cyan/white)
  - `^` (Shift+6): Lava (red/magenta)
  - `&` (Shift+7): Mono (grayscale)
  - `*` (Shift+8): Rainbow
  - `(` (Shift+9): Neon (blue/magenta)
- `Space`: Pause/Resume
- `q` or `Esc`: Quit

## Examples

```bash
# Large bushy bonsai with live animation
termart bonsai -l -L 60 -M 10

# Screensaver mode - infinite bonsai trees
termart bonsai -i -w 5

# Matrix rain with fire colors
termart viz -T matrix  # then press Shift+1

# Peaceful ocean waves
termart viz -T waves

# Classic donut
termart viz -T donut

# eDEX-UI style globe with network activity
termart viz -T globe

# Hexagon honeycomb grid
termart viz -T hex

# Live keyboard heatmap (requires input group access)
termart viz -T keyboard
```

## Notes

### Keyboard Visualization

The keyboard visualization uses Linux evdev to monitor global key events. This requires:
- Read access to `/dev/input/event*` devices
- Usually requires membership in the `input` group: `sudo usermod -aG input $USER`
- Log out and back in after adding yourself to the group

## License

MIT License - see [LICENSE](LICENSE) for details.
