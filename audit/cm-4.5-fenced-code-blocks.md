# Fenced code blocks

- Spec: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
- Status: **keep**
- Corpus: `cm-4.5-a-fences`, `cm-4.5-b-fence-info`, `cm-4.5-c-fence-content`, `cm-4.5-d-fence-in-contexts`
- Walk: same `code_frame` as indented. Backtick and tilde fences both become `Frame::Code`. The fence info string (language) is dropped; `Code` has no field for it. Smart punctuation is not applied inside the literal (`prose_curls_quotes_and_code_stays_straight`).
- Proof: goldens `cm-4.5-a`…`d` (backtick and tilde fences; info strings paint nothing); `spec.rs` code facts; fixture `04-code.md`.
- Later do: none. Language remains unrepresentable on `Code`.
