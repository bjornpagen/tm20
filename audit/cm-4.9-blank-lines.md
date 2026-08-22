# Blank lines

- Spec: https://spec.commonmark.org/0.31.2/#blank-lines
- Status: **keep**
- Corpus: `cm-4.9-a-blank-lines`
- Walk: blank lines are not Frames. They separate paragraphs (`two_paragraphs`) and decide list tightness in comrak (`loose_list`, `blank_between_same_type_items_is_one_loose_list`).
- Proof: those `spec.rs` tests
- Later do: none
