# Indented code blocks

- Spec: https://spec.commonmark.org/0.31.2/#indented-code-blocks
- Status: **keep**
- Corpus: `cm-4.4-a-indented-code`
- Walk: `NodeValue::CodeBlock` → `code_frame`: literal split on `\n`, trailing empty line dropped, `Frame::Code` at body size (8 pt in notes). Info string is empty for indented code.
- Proof: `spec.rs::fenced_and_indented_code`; fixture `04-code.md`
- Later do: none
