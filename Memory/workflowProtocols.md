---
version: "1.0"
lastUpdated: "2025-12-09"
lifecycle: core
stakeholder: pknull
changeTrigger: "workflow pattern changes"
validatedBy: "usage verification"
dependencies: ["techEnvironment.md"]
---

# Workflow Protocols

## Development Workflow

1. **Feature Addition**
   - Add algorithm to `fractal.rs` or new module
   - Register in `FractalType` enum (`config.rs`)
   - Add CLI option in `main.rs`
   - Test interactively

2. **Bug Fixes**
   - Reproduce in debug build
   - Fix with minimal change scope
   - Verify terminal state cleanup

## Testing Protocol

- Manual testing primary (visual output)
- Test terminal resize handling
- Verify clean exit (terminal state restored)
- Check all color schemes render correctly

## Release Process

```bash
cargo build --release
cargo install --path .
# Test installed binary
termart bonsai --live
termart viz -T plasma
```

## Performance Considerations

- Profile with `cargo flamegraph` if slowdown detected
- Buffer operations minimize syscalls
- Avoid allocations in hot render loops
- Use integer math where possible (cube/donut projections)

## Adding New Visualizations

1. Create `run_<name>()` function in `fractal.rs`
2. Follow existing pattern: init state, loop with event polling, render frame
3. Support speed control via delay adjustment
4. Support color scheme via `scheme_color()` helper
5. Add to `FractalType` enum with serde rename
6. Add match arm in `main.rs` dispatch
