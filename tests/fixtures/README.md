# Codex usage fixture

`codex_usage_null_limits.json` reproduces the relevant shape of an HTTP 200
response from `https://chatgpt.com/backend-api/wham/usage` observed on 2026-09-17.
It is reconstructed from field types: identifiers, plan, quota values, and
timestamps are synthetic; unused response fields are omitted. It contains no
auth data. The local Codex CLI was 0.153.4, but the failure was in the usage API
response, not the local `auth.json` schema.

The response contained `additional_rate_limits: null`. The old parser accepted
an omitted field or an array, but rejected null with `invalid type: null,
expected a sequence`. Tests exercise the same `ureq::Response::into_json` path
used by the widget, including the legacy array shape.
