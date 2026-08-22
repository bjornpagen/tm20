# HTML blocks

- Spec: https://spec.commonmark.org/0.31.2/#html-blocks
- Status: **never**
- Corpus: `rej-html-a-block`, `rej-html-b-inline`, `rej-html-c-comment`, `rej-html-d-bare-tag`
- Walk: `NodeValue::HtmlBlock(_) => Err(Error::Html)`. HTML never becomes a Frame (`error.rs`, crate docs).
- Proof: `spec.rs::html_block_and_inline_are_errors` (`<div>no</div>`)
- Later do: none
