# termart Review

## Summary

termart is a well-crafted Rust terminal application providing 20+ ASCII/braille visualizations (bonsai trees, matrix rain, donut, globe with GeoIP), system monitors (CPU, GPU, memory, disk, network), and utilities (weather, pomodoro, FAH stats). At ~20,000 lines of Rust code, it demonstrates solid architecture with good separation of concerns, though it lacks tests entirely and has some error handling gaps that could cause panics in edge cases.

## Critical Issues

- **No Test Coverage**: Zero test files exist (`/home/pknull/Code/termart/tests/` directory missing). For a 20k LOC project with complex cryptographic operations (FAH module), system interactions, and network calls, this is a significant risk.

- **`/home/pknull/Code/termart/src/fah.rs:325-342`**: RSA private key parsing failures are silently logged to `/tmp/fah_debug.log` rather than surfaced to user. Key parsing errors could leave users confused about why remote machines don't connect.

- **`/home/pknull/Code/termart/src/settings.rs:43`**: Config parsing errors (`toml::from_str`) silently return defaults with `unwrap_or_default()`. User may have typos in config that are ignored without warning.

- **`/home/pknull/Code/termart/src/bonsai.rs:21-24`**: `unwrap()` on `duration_since(UNIX_EPOCH)` - will panic if system clock is before Unix epoch (rare but possible on misconfigured systems).

- **`/home/pknull/Code/termart/src/fah.rs:67-101`**: Manual date calculation in `chrono_now_iso()` reimplements what chrono crate (already a dependency) provides, introducing potential bugs and leap year edge cases.

## Recommendations

- **[High]** Add integration tests for critical paths: terminal rendering, config parsing, network error handling. Even basic smoke tests would catch regressions.

- **[High]** Audit `unwrap()` usage (15 occurrences across 6 files) - replace with proper error handling via `?` operator or meaningful error messages.

- **[High]** Surface config parsing errors to stderr with helpful messages instead of silently using defaults.

- **[Medium]** The FAH module stores sensitive credentials (RSA private keys, session tokens) in `~/.config/termart/config.toml` - document that file permissions should be restricted (0600).

- **[Medium]** Consider using `chrono` crate consistently for date/time instead of manual calculations in `fah.rs:67-101`.

- **[Medium]** The `debug_log!` macro writes to `/tmp/fah_debug.log` unconditionally with `create(true)` - this could fill disk over time if FAH module is used frequently.

- **[Low]** Add `#![deny(clippy::unwrap_used)]` to catch future unwrap additions at compile time.

- **[Low]** The globe visualization uses HTTP (not HTTPS) for `ip-api.com` geolocation (`/home/pknull/Code/termart/src/viz/globe.rs:226`) - comment documents this but consider noting the privacy implications.

- **[Low]** Consistent error messages - some modules show errors to user (weather, fah), others fail silently (settings, some network calls).

## Scores (1-10)

- Code Quality: 7
- Architecture: 8
- Completeness: 5
- Standards: 7

## Notes

**Strong points:**
- Clean module organization: `viz/`, `monitor/` subdirectories with consistent patterns
- Good use of Rust idioms: `LazyLock` for static data, builder patterns for config
- Terminal abstraction (`Terminal` struct) with double-buffering for efficient rendering
- Consistent keyboard handling via `VizState` and `ColorState` shared across visualizations
- Comprehensive CLI with clap derive macros - well documented with examples
- README is excellent with usage examples, option tables, and feature descriptions

**Concerns:**
- The FAH module (`fah.rs`, 1518 lines) is extremely complex with WebSocket handling, RSA encryption, AES-CBC, PBKDF2 - this is the highest risk code with no tests
- Password derivation uses 100,000 PBKDF2 iterations which is reasonable but should be documented
- Network operations block the main thread (ureq is synchronous) - could freeze UI on slow networks
- No graceful degradation if system APIs fail (e.g., /proc filesystem unavailable)

**Missing features (based on README promises):**
- Audio visualizer depends on `cpal` which may not work without audio hardware
- Keyboard visualizer requires `/dev/input` access - needs root or input group membership

**Dependencies (34 direct):**
- Heavy crypto stack: `rsa`, `aes`, `cbc`, `pbkdf2`, `sha2` - all well-maintained crates
- `evdev` for keyboard capture is Linux-only
- `serialport` for Dygma keyboard is likely cross-platform but untested on non-Linux
