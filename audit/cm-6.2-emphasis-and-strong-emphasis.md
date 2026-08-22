# Emphasis and strong emphasis

- Spec: https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis
- Status: **keep**
- Corpus: `cm-6.2-a-flanking`, `cm-6.2-b-intraword`, `cm-6.2-c-mixed-delims`, `cm-6.2-d-adjacent-runs`, `cm-6.2-e-punct-flanks`
- Walk: `Emph` / `Strong` nest `Voice` into `Cut::Italic`, `Bold`, `BoldItalic`. Delimiter run rules are comrak’s.
- Proof: goldens `cm-6.2-a`…`e` (`cm-6.2-b` is the underscore intra-word fact); `spec.rs::emphasis_and_strong`; fixture `08-emphasis.md`.
- Later do: none.
