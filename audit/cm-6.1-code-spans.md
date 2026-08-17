# Code spans

- Spec: https://spec.commonmark.org/0.31.2/#code-spans
- Status: **keep**
- Walk: `NodeValue::Code` → `Cut::Mono` via `strip_code` (one space stripped from both ends when the span is not all spaces, matching the spec). Voice around the span is not applied to the mono run.
- Proof: `spec.rs::code_span_is_mono`; fixture `08-emphasis.md`
- Later do: none
