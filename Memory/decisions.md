# Decisions

- Token quota health is measured against the grey elapsed-time pacing boundary whenever that boundary is available, not against an unconditional percentage threshold.
- The pacing warning band begins 15 percentage points before the boundary. The boundary itself remains yellow; only usage strictly beyond it is red.
- When pacing data is unavailable, quota colors fall back to green below 50%, yellow from 50% through 80%, and red above 80%.
- Claude and Codex token widgets share the same quota-bar rendering and color semantics through `src/viz/usage.rs`.
- Anthropic quotas are rendered from the dynamic `limits` list using API-provided scoped display names, with legacy fixed fields retained only as compatibility fallbacks.

- Headroom history graphs are right-anchored: the newest sample occupies the
  rightmost column, older samples shift left, and a partially filled buffer
  renders flush right with the empty region on the left. Sample history lives
  in a bounded fixed-capacity buffer that evicts the oldest sample at capacity
  and never grows without bound.

- Availability metrics colour by headroom, not load, and reuse
  `headroom_gradient_color_scheme` rather than duplicating thresholds: high
  availability renders green and low renders red, inverting the load ramp.

- Memory/ is exactly the v2 pair (activeContext.md, decisions.md); reference
  material lives in docs/ and README, machine-local state under ignored Work/.
