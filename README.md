# tm20

Markdown → thermal tape for the Epson TM-T20III: 576 dots, 80 mm, 203 dpi.
A strict printable subset of CommonMark and GFM. Unsupported constructs,
missing glyphs, and clipped content are errors with source context—not fallbacks.
Code and tests define behavior.

| Feature | Supported behavior | Specification |
| --- | --- | --- |
| Paragraphs | 11 pt Helvetica; word wrapping, no hyphenation | [CommonMark](https://spec.commonmark.org/0.31.2/#paragraphs) |
| Emphasis / strong | Oblique / bold; combinations use bold oblique | [CommonMark](https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis) |
| Headings | H1: 18 pt; H2–H6: 11 pt bold. Plain text only; empty or styled headings reject | [ATX](https://spec.commonmark.org/0.31.2/#atx-headings), [Setext](https://spec.commonmark.org/0.31.2/#setext-headings) |
| Soft / hard breaks | Soft breaks become spaces; hard breaks start a new line | [Soft](https://spec.commonmark.org/0.31.2/#soft-line-breaks), [hard](https://spec.commonmark.org/0.31.2/#hard-line-breaks) |
| Escapes / entities | Decoded text; glyph coverage depends on supplied fonts | [Escapes](https://spec.commonmark.org/0.31.2/#backslash-escapes), [entities](https://spec.commonmark.org/0.31.2/#entity-and-numeric-character-references) |
| Code spans | Menlo; normalized whitespace; fitting spans stay unbroken | [CommonMark](https://spec.commonmark.org/0.31.2/#code-spans) |
| Code blocks | Fenced or indented; Menlo, no highlighting or wrapping; overwide lines reject | [Fenced](https://spec.commonmark.org/0.31.2/#fenced-code-blocks), [indented](https://spec.commonmark.org/0.31.2/#indented-code-blocks) |
| Lists | Dash bullets; ordered starts and delimiters preserved; tight/loose spacing; nesting ≤3 | [CommonMark](https://spec.commonmark.org/0.31.2/#lists) |
| Task lists | Checked and unchecked boxes | [GFM](https://github.github.com/gfm/#task-list-items-extension-) |
| Block quotes | Indented; nesting ≤3, independent of list depth | [CommonMark](https://spec.commonmark.org/0.31.2/#block-quotes) |
| Thematic breaks | Two-dot rule across the full tape, including inside containers | [CommonMark](https://spec.commonmark.org/0.31.2/#thematic-breaks) |
| Tables | Two/three columns only; header-only tables accepted; ragged rows reject | [GFM](https://github.github.com/gfm/#tables-extension-) (restricted) |
| Table alignment | Left/right; centered columns reject. Right-aligned cells use tabular digits; clipped content rejects | [GFM](https://github.github.com/gfm/#tables-extension-) (restricted) |
| Table pipes | Literal pipes must be escaped, including inside code spans | [GFM](https://github.github.com/gfm/#tables-extension-) |
| Links | Inline/reference links; italic labels, numbered destination endnotes when nonredundant; destinations deduplicate | [CommonMark](https://spec.commonmark.org/0.31.2/#links) |
| Autolinks | Angle links, recognized bare URLs and email; normally no redundant endnote | [CommonMark](https://spec.commonmark.org/0.31.2/#autolinks), [GFM](https://github.github.com/gfm/#autolinks-extension-) |
| Images | Standalone PNG/JPEG; local files or HTTP(S) via system curl; shrink to local width, dither to monochrome; no printed alt text | [CommonMark](https://spec.commonmark.org/0.31.2/#images) (restricted) |
| Raw HTML | Rejected, including comments; escaped HTML and code literals remain text | [CommonMark](https://spec.commonmark.org/0.31.2/#raw-html), [HTML blocks](https://spec.commonmark.org/0.31.2/#html-blocks) (unsupported) |
| Strikethrough | Strike line across text, including nested styles and wrapped lines | [GFM](https://github.github.com/gfm/#strikethrough-extension-) |
| Footnotes | Named references and multiblock definitions; first-use numbering shared with link notes; unused omitted, undefined references reject | Extension outside CommonMark/GFM |
| Math | LaTeX via RaTeX: `\(inline\)`, `\[display\]`; display math outside inline styling/tables; dollar delimiters disabled; unsupported formulas reject | Extension outside CommonMark/GFM |
| Smart punctuation | Curly quotes, en/em dashes, ellipses in prose | Extension outside CommonMark/GFM |
| Other extensions | No YAML metadata, CSS, definition lists, or image-size attributes | Not enabled |

| Crate | Responsibility |
| --- | --- |
| [tm20](crates/tm20/src/lib.rs) | ESC/POS encoding, raster bands, barcodes/symbols, USB/serial/TCP/memory transports. CODE128-C encoding and DataMatrix model support remain unverified |
| [tm20-set](crates/tm20-set/src/lib.rs) | Typed sheets, font shaping, layout, rasterization, banding, PNG previews |
| [tm20-md](crates/tm20-md/src/lib.rs) | Strict Markdown parsing, source diagnostics, math and local image loading |
| [tm20-cli](crates/tm20-cli/src/main.rs) | `tm20-set` executable: prepare, preview, print; macOS Helvetica/Menlo fonts |

Agent workflow: [SKILL.md](.claude/skills/tm20/SKILL.md).

Parallel tests: `cargo nextest run --workspace`. Doctests: `cargo test --workspace --doc`.
