# tm20-md audit

Dump only. No crate changes in this pass. Parser is comrak
([CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/)) with
`table`, `tasklist`, `autolink`, `footnotes`, `math_latex`, and
`parse.smart` in `crates/tm20-md/src/lower.rs`. Each file is one
construct.

## Status

- **keep** — lowered, and a test, fixture, or snap golden owns the fact
- **never** — unrepresentable on purpose
- **unproven** — comrak + walk likely handle it; no dedicated test
- **gap** — errors, drops, or collapses in a way that is not an explicit never

Each construct file has a `Corpus:` line naming the snap stems in
`crates/tm20-md/tests/corpus/` (and `reject/` where the fact is a hard
error). Band-split and pair-matrix stems (`set-*`, `doc-*`) live only
in the snap suite; they have no audit file.

## CommonMark

- [cm-2.1-characters-and-lines.md](cm-2.1-characters-and-lines.md)
- [cm-2.2-tabs.md](cm-2.2-tabs.md)
- [cm-2.3-insecure-characters.md](cm-2.3-insecure-characters.md)
- [cm-2.4-backslash-escapes.md](cm-2.4-backslash-escapes.md)
- [cm-2.5-entity-and-numeric-character-references.md](cm-2.5-entity-and-numeric-character-references.md)
- [cm-3.1-precedence.md](cm-3.1-precedence.md)
- [cm-3.2-container-blocks-and-leaf-blocks.md](cm-3.2-container-blocks-and-leaf-blocks.md)
- [cm-4.1-thematic-breaks.md](cm-4.1-thematic-breaks.md)
- [cm-4.2-atx-headings.md](cm-4.2-atx-headings.md)
- [cm-4.3-setext-headings.md](cm-4.3-setext-headings.md)
- [cm-4.4-indented-code-blocks.md](cm-4.4-indented-code-blocks.md)
- [cm-4.5-fenced-code-blocks.md](cm-4.5-fenced-code-blocks.md)
- [cm-4.6-html-blocks.md](cm-4.6-html-blocks.md)
- [cm-4.7-link-reference-definitions.md](cm-4.7-link-reference-definitions.md)
- [cm-4.8-paragraphs.md](cm-4.8-paragraphs.md)
- [cm-4.9-blank-lines.md](cm-4.9-blank-lines.md)
- [cm-5.1-block-quotes.md](cm-5.1-block-quotes.md)
- [cm-5.2-list-items.md](cm-5.2-list-items.md)
- [cm-5.3-lists.md](cm-5.3-lists.md)
- [cm-6.1-code-spans.md](cm-6.1-code-spans.md)
- [cm-6.2-emphasis-and-strong-emphasis.md](cm-6.2-emphasis-and-strong-emphasis.md)
- [cm-6.3-links.md](cm-6.3-links.md)
- [cm-6.4-images.md](cm-6.4-images.md)
- [cm-6.5-autolinks.md](cm-6.5-autolinks.md)
- [cm-6.6-raw-html.md](cm-6.6-raw-html.md)
- [cm-6.7-hard-line-breaks.md](cm-6.7-hard-line-breaks.md)
- [cm-6.8-soft-line-breaks.md](cm-6.8-soft-line-breaks.md)
- [cm-6.9-textual-content.md](cm-6.9-textual-content.md)

## Opted-in extras

- [extra-pipe-tables.md](extra-pipe-tables.md)
- [extra-task-lists.md](extra-task-lists.md)
- [extra-gfm-autolink.md](extra-gfm-autolink.md)
- [extra-footnotes.md](extra-footnotes.md)
- [extra-latex-math.md](extra-latex-math.md)
- [extra-smart-punctuation.md](extra-smart-punctuation.md)

## Refused

- [never-strikethrough.md](never-strikethrough.md)
- [never-math-dollars.md](never-math-dollars.md)
- [never-options-gfm.md](never-options-gfm.md)
- [never-raw-html-as-frame.md](never-raw-html-as-frame.md)
