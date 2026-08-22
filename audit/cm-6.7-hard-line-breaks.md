# Hard line breaks

- Spec: https://spec.commonmark.org/0.31.2/#hard-line-breaks
- Status: **keep**
- Corpus: `cm-6.7-a-hard-breaks`
- Walk: `NodeValue::LineBreak` → `"\n"` in the current cut (`push`). Trailing two spaces or a backslash break are comrak’s.
- Proof: `spec.rs::hard_and_soft_breaks`
- Later do: none
