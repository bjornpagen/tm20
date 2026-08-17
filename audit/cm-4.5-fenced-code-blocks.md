# Fenced code blocks

- Spec: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
- Status: **gap**
- Walk: same `code_frame` as indented. Backtick and tilde fences both become `Frame::Code`. The fence info string (language) is dropped; `Code` has no field for it. Smart punctuation is not applied inside the literal (`prose_curls_quotes_and_code_stays_straight`).
- Proof: `spec.rs::fenced_and_indented_code`, `prose_curls_quotes_and_code_stays_straight`; fixture `04-code.md`. No tilde-fence or info-string proof.
- Later do: keep dropping language (closed `Code`) and add a tilde-fence test, or store the info string if a later Frame wants it.
