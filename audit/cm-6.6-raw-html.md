# Raw HTML

- Spec: https://spec.commonmark.org/0.31.2/#raw-html
- Status: **never**
- Walk: `NodeValue::HtmlInline(_) => Err(Error::Html)`. Same as HTML blocks. Unknown `NodeValue` in `inline` also returns `Error::Html`.
- Proof: `spec.rs::html_block_and_inline_are_errors` (`a <span>b</span> c`)
- Later do: none
