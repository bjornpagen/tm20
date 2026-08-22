# Insecure characters

- Spec: https://spec.commonmark.org/0.31.2/#insecure-characters
- Status: **keep**
- Corpus: `cm-2.3-a-insecure`
- Walk: `sheet` maps leftover U+0000 → U+FFFD before comrak so the tape shows a replacement, not a zero-width hole.
- Proof: golden `cm-2.3-a-insecure`; `spec.rs` NUL fact.
- Later do: none.
