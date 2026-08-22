# Soft line breaks

- Spec: https://spec.commonmark.org/0.31.2/#soft-line-breaks
- Status: **keep**
- Corpus: `cm-6.8-a-soft-breaks`
- Walk: `NodeValue::SoftBreak` → a space, not a newline. Compose wraps on the typesetter grid, not on the markdown line.
- Proof: `spec.rs::hard_and_soft_breaks`
- Later do: none
