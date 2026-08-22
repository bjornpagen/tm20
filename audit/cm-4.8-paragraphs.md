# Paragraphs

- Spec: https://spec.commonmark.org/0.31.2/#paragraphs
- Status: **keep**
- Corpus: `cm-4.8-a-paragraphs`
- Walk: `NodeValue::Paragraph` → `paragraph`: a sole image child is `Frame::Figure`; any other image in the paragraph is `Error::MixedImage`; display math flushes a `Frame::Math`; remaining inlines become `Frame::Text`.
- Proof: `spec.rs::empty_and_paragraph`, `two_paragraphs`; fixture `01-prose.md`
- Later do: none for CM paragraphs themselves (image mix is `cm-6.4`)
