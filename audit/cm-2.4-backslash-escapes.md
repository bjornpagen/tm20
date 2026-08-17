# Backslash escapes

- Spec: https://spec.commonmark.org/0.31.2/#backslash-escapes
- Status: **keep**
- Walk: comrak emits `NodeValue::Escaped` or decoded text; `inline` walks `Escaped` children with the current voice (`lower.rs` `NodeValue::Escaped`).
- Proof: `spec.rs::backslash_and_entity` (`\*star\*` → `*star*`); fixture `01-prose.md`
- Later do: none unless a punctuation class is missing from a proof.
