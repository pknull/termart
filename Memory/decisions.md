# Decisions

- Token quota health is measured against the grey elapsed-time pacing boundary whenever that boundary is available, not against an unconditional percentage threshold.
- The pacing warning band begins 15 percentage points before the boundary. The boundary itself remains yellow; only usage strictly beyond it is red.
- When pacing data is unavailable, quota colors fall back to green below 50%, yellow from 50% through 80%, and red above 80%.
- Claude and Codex token widgets share the same quota-bar rendering and color semantics through `src/viz/usage.rs`.
- Anthropic quotas are rendered from the dynamic `limits` list using API-provided scoped display names, with legacy fixed fields retained only as compatibility fallbacks.
