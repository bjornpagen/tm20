# Emphasis and strong emphasis

- Spec: https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis
- Status: **keep**
- Walk: `Emph` / `Strong` nest `Voice` into `Cut::Italic`, `Bold`, `BoldItalic`. Delimiter run rules are comrak’s. Underscore emphasis is unproven.
- Proof: `spec.rs::emphasis_and_strong`; fixture `08-emphasis.md`
- Later do: `_italic_` and `__bold__` proofs.
