# HTML blocks

- Spec: https://spec.commonmark.org/0.31.2/#html-blocks
- Status: **never**
- Walk: `NodeValue::HtmlBlock(_) => Err(Error::Html)`. HTML never becomes a Frame (`error.rs`, crate docs).
- Proof: `spec.rs::html_block_and_inline_are_errors` (`<div>no</div>`)
- Later do: none
