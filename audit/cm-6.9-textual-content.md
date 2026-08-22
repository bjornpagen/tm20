# Textual content

- Spec: https://spec.commonmark.org/0.31.2/#textual-content
- Status: **keep**
- Corpus: `cm-6.9-a-unicode`, `cm-6.9-b-scripts`, `cm-6.9-c-tofu`
- Walk: `NodeValue::Text` → `Span::Type` at the current `Cut`. Adjacent same-cut runs merge (`push`). Empty document is an empty `Sheet`.
- Proof: `spec.rs::empty_and_paragraph`; fixture `01-prose.md`
- Later do: none
