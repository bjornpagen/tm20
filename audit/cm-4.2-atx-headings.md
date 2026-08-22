# ATX headings

- Spec: https://spec.commonmark.org/0.31.2/#atx-headings
- Status: **keep**
- Corpus: `cm-4.2-a-atx-levels`, `cm-4.2-b-atx-forms`, `cm-4.2-c-atx-flatten`
- Walk: `heading`: math in a heading is `Error::Math`. Inlines flatten to a string (`flatten`). Level `<= 1` → `Frame::Mark` at 18 pt. Any other level → `Frame::Head` at 11 pt. Levels 3–6 are the same Head as level 2. Closing hashes are stripped by comrak.
- Proof: goldens `cm-4.2-a-atx-levels` (one masthead, five identical bold heads), `cm-4.2-b-atx-forms`, `cm-4.2-c-atx-flatten`; `spec.rs` heading facts; fixture `02-heads.md`.
- Later do: none. Levels 3–6 collapsing to Head is the dialect — there is no size ladder.
