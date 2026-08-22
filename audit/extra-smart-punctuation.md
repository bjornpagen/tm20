# Smart punctuation

- Spec: not CommonMark; `options.parse.smart`
- Status: **keep**
- Corpus: `ext-smart-a-quotes`, `ext-smart-b-dashes`
- Walk: comrak curls quotes and turns `'` / `"` in prose before the walk. Code spans and fenced literals stay straight.
- Proof: `spec.rs::prose_curls_quotes_and_code_stays_straight`; fixture `01-prose.md`
- Later do: none
