---
name: tm20
description: Author or revise Markdown receipts, reading tapes, and checklists for tm20. Use its supported Markdown subset and narrow monochrome design language; avoid constructs that cannot render faithfully.
---

# Design Markdown for thermal tape

Craft a `.md` document for a 576-dot-wide, monochrome thermal tape. Length can
grow; width cannot. tm20 is a strict printable subset of CommonMark/GFM, not
a browser: unsupported constructs, missing glyphs, and clipped content fail.

## Design language

- Prefer one short `#` masthead, then a clear reading order: context, substance,
  conclusion. Use `##` for sections; H3–H6 do not create smaller visual levels.
- Be economical, not cryptic. Use short paragraphs and concrete labels. Keep
  necessary detail; move supporting sources into notes rather than deleting it.
- Let typography do the work: bold for key facts, italic for secondary emphasis,
  monospace for literal identifiers. Avoid walls of bold, all-caps paragraphs,
  decorative emoji, ASCII boxes, and space-padded pseudo-columns.
- Use two-column tables for label/value pairs and prices. Reserve three columns
  for genuinely compact data. Prefer stacked labeled paragraphs for wide records.
- Use a rule before a total or major transition, not between every paragraph.
  One blank line separates blocks; extra blank lines are not layout controls.
- Receipts: masthead → context → items → total. Reading tapes: short sections
  and prose. Checklists: one concrete action per task. Adapt the structure to
  the content; do not force every document into a receipt.

## Supported Markdown

| Construct | Rendering | Authoring constraints |
| --- | --- | --- |
| Paragraphs | 11 pt sans; word wrapping, no hyphenation | Avoid long unbroken strings. Source soft breaks become spaces; use a trailing backslash or two spaces for a hard break. |
| Headings | H1: 18 pt; H2–H6: 11 pt bold | Nonempty plain text only: no emphasis, code, links, images, or math. Prefer ATX `#` syntax. |
| Inline styles | `*italic*`, `**bold**`, combinations, `~~strike~~` | Styles can nest; strikethrough spans wrapped lines. Keep them out of headings. |
| Code spans | Monospace; whitespace normalized | For short literals, not manual alignment. Fitting spans stay unbroken. |
| Code blocks | Fenced or indented monospace | No highlighting or wrapping. Split long lines explicitly; indentation consumes width. |
| Lists | Dash bullets; ordered starts and `.` / `)` delimiters preserved | At most three list levels. Blank lines distinguish loose from tight lists. |
| Tasks | `- [ ]` and `- [x]` boxes | Use list-item syntax, not free-standing bracket decorations. |
| Quotes | Indented blocks | At most three quote levels, counted separately from list levels. Nesting reduces usable width. |
| Rules | Full-tape two-dot line | Put blank lines around `---`; immediately beneath text it can become a Setext heading. |
| Tables | Two or three columns; bold header; left/right alignment | Use `---` or `---:`; never centered `:---:`. Every row needs exactly the header's cell count. Cells contain inline content, not nested blocks. |
| Links | Italic labels; numbered destination endnotes when needed | Inline, reference, angle, and recognized bare links work. Define references; use consistent titles for repeated destinations. Long URLs can overflow even in notes. |
| Footnotes | First-use numbering shared with link notes; multiblock definitions | Define every `[^name]`. Unused definitions disappear. Indent continuation blocks. |
| Images | Standalone PNG/JPEG, shrunk to fit and dithered; never upscaled | Image alone in its paragraph, not inside a link or table. Prefer local assets; remote URLs need permission outside the document. Alt text is not printed: put meaningful captions in a separate paragraph. |
| Math | LaTeX via RaTeX: `\(inline\)` and `\[display\]` | Dollars are currency, not delimiters. No heading math; display math belongs in a separate paragraph, outside styles, links, and tables. Unsupported formulas/glyphs fail. |
| Text conventions | Escapes/entities decoded; smart quotes, dashes, ellipses in prose | Use code for literal punctuation. Glyph coverage is finite; do not assume emoji or arbitrary scripts are available. |

## Footguns to avoid

- No raw HTML, including comments or `<br>`. No CSS, YAML front matter,
  definition-list extension, image-size attributes, or browser layout tricks.
- Escape literal table pipes as `\|`, even inside code spans. Backticks alone
  do not protect a pipe from splitting a cell.
- Escape literal square brackets (`\[` and `\]`) when they are not links,
  footnotes, or tasks; apparent references without definitions reject.
- Keep amount columns right-aligned with consistent decimal precision. Their
  digits are tabular, but there is no spreadsheet-style number formatting.
- Narrow a table by shortening labels or moving detail into prose, not by
  dropping data. Missing glyphs or overflow are not invitations to invent
  substitutes or silently omit content.

## Receipt idiom

A header-only table after a rule gives the total the same alignment as the
items, with automatic header emphasis. Keep the separator row even without
body rows:

```markdown
# Corner shop

Order 42\
5 September 2026

| Item | Amount |
| --- | ---: |
| Coffee | 6.00 |
| Bread | 4.50 |

---

| Total | 10.50 |
| --- | ---: |

Thank you.
```
