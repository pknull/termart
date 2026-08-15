# Objective

- Make the Claude and Codex quota graphs communicate pacing clearly while retaining every quota returned by each provider.

# State

- `src/viz/tokeneater.rs` and `README.md` contain uncommitted dynamic Anthropic quota support. API-named scoped limits such as Fable render alongside legacy buckets, and null legacy model buckets are omitted.
- `src/viz/usage.rs` contains uncommitted shared color-band logic for both token widgets. Usage stays green while more than 15 percentage points below the grey pacing boundary, turns yellow within that margin through the boundary, and turns red only after crossing it.
- Quotas without pacing data use fixed fallback bands: green below 50%, yellow from 50% through 80%, and red above 80%.
- The release binary is installed at the active asdf Rust path and matches `target/release/termart`. The Claude and Codex token widgets were restarted in tmux panes `0:0.14` and `0:0.15` and rendered successfully.
- Verification is green: standard Rust verification passed format, tests, check, and clippy; `cargo test --all-targets` passed 61 unit and 5 integration tests.

# Next

- Review the live color transitions as usage approaches and crosses the grey pacing boundary.
- Commit the source and README changes separately when approved; they remain intentionally uncommitted.

# Blockers

- None.
