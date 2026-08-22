# Setext headings

- Spec: https://spec.commonmark.org/0.31.2/#setext-headings
- Status: **keep**
- Corpus: `cm-4.3-a-setext`
- Walk: same `heading` as ATX. `=` is level 1 → `Mark`. `-` is level 2 → `Head`.
- Proof: `spec.rs::atx_and_setext` (`Setext` / `======` → `Mark`)
- Later do: a setext h2 (`---`) so it cannot be confused with a thematic break.
