# Footnotes

- Spec: not CommonMark; `options.extension.footnotes`
- Status: **keep**
- Corpus: `ext-foot-a-basic`, `ext-foot-b-multiblock`, `ext-foot-c-order`, `ext-foot-d-in-cell`, `ext-foot-e-in-quote`
- Walk: `FootnoteDefinition` is collected at 8 pt and omitted from the body. `FootnoteReference` shares the note registry with link dests. Undefined `[^x]` stays literal (comrak does not emit a reference). `Error::Note` if a reference was counted but the definition is missing at materialize.
- Proof: `spec.rs::footnotes_share_the_link_registry`, `footnote_reuses_the_number`, `undefined_footnote_stays_literal`; fixture `09-notes.md`
- Later do: none
