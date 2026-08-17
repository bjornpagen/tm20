# Characters and lines

- Spec: https://spec.commonmark.org/0.31.2/#characters-and-lines
- Status: **unproven**
- Walk: UTF-8 source is handed to comrak; `Cx::inline` copies `NodeValue::Text` into `Span::Type`. No extra line-ending or Unicode-space policy in the walk.
- Proof: none (only ASCII snippets in `spec.rs`)
- Later do: a test that a CR LF pair is one break and that a non-ASCII letter survives into a span.
