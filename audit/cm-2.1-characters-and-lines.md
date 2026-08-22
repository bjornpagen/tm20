# Characters and lines

- Spec: https://spec.commonmark.org/0.31.2/#characters-and-lines
- Status: **keep**
- Corpus: `cm-2.1-a-line-endings`
- Walk: UTF-8 source is handed to comrak; `Cx::inline` copies `NodeValue::Text` into `Span::Type`. No extra line-ending or Unicode-space policy in the walk.
- Proof: golden `cm-2.1-a-line-endings` (CRLF and lone CR compose as LF).
- Later do: none.
