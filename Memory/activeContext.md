# Objective

Make the quota graphs and the system monitor communicate pacing and headroom
at a glance, retaining every quota each provider returns.

# State

- Dynamic Anthropic quota support is committed (`68acf185`). Quotas render from
  the endpoint's `limits` list using API-provided scoped display names, so a
  model-scoped limit such as Fable appears without a field per model; null
  legacy buckets are omitted, and both token widgets share the same bar
  rendering and colour semantics.
- Right-anchored headroom history graphs are committed (`55654101`),
  integrated from the Asha initiative `monitor-headroom-history-graphs` (seal
  `2576826e`, review `accepted-pass`, verification `passed` under a minimal
  environment). A bounded fixed-capacity sample buffer feeds a reusable
  renderer in `src/monitor/layout.rs`; the memory Available row and the disk
  available rows each draw a graph beside their existing meter, coloured by
  `headroom_gradient_color_scheme` so high availability reads green and low
  reads red.
- Verification is green on the current tree: `cargo build --release`,
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets` (75 unit and 5 integration tests).
- Memory v2 publication is valid with stable project id
  `d911d695-6af2-4096-a31b-8c16cb6b1cc9`.

# Next

- Watch the live colour transitions as usage approaches and crosses the grey
  pacing boundary.
- Confirm the history graphs read correctly at small terminal heights and while
  the sample buffer is only partially filled.
- Install the freshly built release binary over the active asdf Rust path and
  restart the token widgets when convenient.

# Blockers

- None.
