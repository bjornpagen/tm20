# Raw HTML as a Frame

- Spec: CommonMark 4.6 and 6.6, refused as typesetting.
- Status: **never**
- Walk: `HtmlBlock` and `HtmlInline` → `Error::Html`. Crate docs: HTML never becomes a Frame. Unknown block/inline nodes use the same error.
- Proof: `spec.rs::html_block_and_inline_are_errors`; `error.rs` `Error::Html`
- Later do: none
