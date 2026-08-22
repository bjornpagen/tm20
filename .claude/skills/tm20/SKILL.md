---
name: tm20
description: Print markdown on the Epson TM-T20III receipt printer and author markdown in the tm20 dialect. Use when the user asks to print something, make a receipt, ticket, or tape, preview a print as PNG, or write markdown destined for the thermal printer.
---

# tm20: author and print tape-sized markdown

The pipeline: markdown → comrak AST → `Sheet` of `Frame`s → one 576-dot-wide
1-bit raster set in Helvetica and Menlo → ESC/POS `GS ( L` graphics →
USB. The printer never sees a font. The whole file encodes **before** the USB
device opens, so a file the dialect rejects costs zero paper.

**Policy: when the user asks to print, print immediately.** Do not insert a
preview step they did not ask for. Preview only when they ask for one, or when
diagnosing a failed encode.

## Commands

Run from the workspace root (`Cargo.toml` with the `tm20-cli` member).

Print one file:

```bash
cargo run --bin tm20-set -- print md path/to/file.md
```

Print every `*.md` in a directory (sorted by filename, one cut per file):

```bash
cargo run --bin tm20-set -- print md path/to/dir
```

Preview without a printer — single file only; writes `<stem>.png` at 2× and
reports band heights and byte count. Read the PNG to check the layout:

```bash
cargo run --bin tm20-set -- --dry --png "$TMPDIR" print md path/to/file.md
```

- `--png DIR` without `--dry` prints *and* saves the preview.
- `--dry` on a directory only lists byte counts; it writes no PNGs. Preview
  files one at a time.
- `--serial S` picks a printer when several TM-T20IIIs are attached.
- Built-in demo sheets: `cargo run --bin tm20-set -- print ticket|helvetica|all`
  (bare `print` lists the catalog). Protocol exerciser: `cargo run -p tm20 -- hello`.

Prerequisites: macOS `/System/Library/Fonts/Helvetica.ttc` and
`/System/Library/Fonts/Menlo.ttc`, and for real prints a TM-T20III
at USB `04b8:0e28`.

## The dialect: what markdown becomes

Body is 11 pt Helvetica on a 576-dot (72 mm, 203 dpi) tape, flush left,
ragged right. 8 dots ≈ 1 mm of paper.

| You write | It prints as |
| --- | --- |
| `# Title` (one per tape) | 18 pt Helvetica display masthead — ~20 characters per line, then it wraps |
| `##` through `######` | all identical: 11 pt Helvetica **Bold** — there is no size ladder |
| paragraph | 11 pt Roman, ~36–42 chars/line |
| `*em*` / `**strong**` / `***both***` | Oblique / Bold / BoldOblique |
| `` `code` `` | Menlo inline |
| fenced or indented code block | mono block, hung 8 dots, tabs expand to 8-col stops, **never wraps** — ~30 chars, then clips |
| `---` on its own | full-measure rule, 2 dots thick, one module of air above and below (kisses a masthead or a table total) |
| `- item` | dash-marked list; `1.` / `3)` ordered (start and delimiter honored) |
| `- [ ]` / `- [x]` | checkbox tasks |
| `> quote` | hangs 16 dots so the voice reads; each nest adds 8 |
| 2–3 column pipe table | aligned columns, bold header; `---:` right-aligns (use for numbers); `:---:` is left, not center |
| inside a table cell | the full inline dialect works: bold, code, links, footnote refs; escape a literal pipe with a backslash; a pipe inside a code span stays cell content |
| `[text](url)` | italic text + superscript number; URL (and `"title"`) print as a numbered endnote at 8 pt; duplicate URLs share one number |
| `<https://…>` or a bare URL — `www.…` included | italic, no endnote (the URL is already on the page) |
| `<a@b.com>` | italic address, no note; `mailto:` is stripped in notes |
| `[^x]` footnote | endnote in the same number sequence as links, 8 pt; unused definitions are dropped silently |
| `\(E=mc^2\)` / `\[ ... \]` | real typeset math, inline / display (RaTeX) |
| `![alt](local.png)` alone in a paragraph | Floyd–Steinberg dithered figure, centered; shrunk if wider than 576, **never upscaled**; alt text never prints; transparency composites onto paper |
| `"quotes"`, `--`, `---`, `...` inline | smart punctuation: curled quotes, en dash, em dash, ellipsis |
| `~~strike~~` | literal tildes (extension off, on purpose) |
| `$4.50` | literal dollars — `$` is never math |

Heading interiors are flattened to plain text: emphasis, code, and links
inside a heading lose their styling (a link there prints as bare words, no
note). Math in a heading is a hard error. Setext headings work (`===` →
masthead, `---` underline → bold head). Escaped `\[x\]` is display math —
`math_latex` owns `\[…\]` — so literal brackets are just `[x]`. Overlong URLs
in prose and notes break after URI punctuation; a token with no punctuation
still clips, honestly. Tall inline math like `\(\frac{a}{b}\)` grows its own
line rather than colliding with the next.

## Hard errors — the whole job fails, nothing prints

| Error message | Cause → fix |
| --- | --- |
| `raw HTML is not representable` | Any HTML at all: `<!-- comments -->`, `<br>`, `<sub>`, `<div>`, badge `<img>`. Delete it; there is no escape hatch. |
| `a paragraph cannot mix text and an image` | An image must be alone in its own paragraph, blank lines on both sides. |
| `table must have two or three columns` | Also fired by a ragged row. Restructure to 2–3 columns; every row exactly that many cells. |
| `could not typeset math` | Math in a heading, or LaTeX RaTeX cannot set. Move it to body text; simplify. |
| `quote or list nested more than three deep` | Flatten to ≤3 levels. |
| `footnote has no definition` | Every `[^x]` needs `[^x]: …`. |
| `could not load figure` | Remote URL (HTTP is refused), a missing file, or bytes that are not PNG/JPEG. Paths resolve relative to the .md file; absolute and `file:` paths work. |
| `…Helvetica.ttc not on this machine` / `…Menlo.ttc not on this machine` | Install the face; both live in `/System/Library/Fonts`. |
| USB open failure | Printer off, unplugged, or claimed. `--dry` still works. |

Also unrepresentable: YAML front matter is not parsed — a leading `---` block
prints as a rule plus stray text. Never emit front matter.

## House style — write for the tape

- One `#` masthead, first line, a few short words. Sections are `##` plus
  prose; put `---` between major movements. Never reach for `###` expecting a
  smaller size — hierarchy is one masthead, flat bold heads, rules, and white.
- The measure is about six words. Short sentences; front-load every line.
- White is adjacency, not a skip you type: exactly one blank line between
  blocks. Extra blank lines collapse; there is no manual vertical space.
  Loose vs tight lists are the only optional air.
- A hard break (trailing `\`) stacks address-style lines inside one block.
- Tables: 2 columns for label/value, 3 for item/qty/price. Right-align every
  numeric column with `---:`, and give money a fixed number of decimals —
  `9.00`, never `9` — because right-aligned columns share a right edge, not a
  decimal point. Adjacent numeric columns take a double gutter on their own.
- Prefer meaningful link text — the URL rides along as a numbered endnote,
  deduplicated across the tape; a `"title"` on the link prints above the URL
  as its caption. Use a bare autolink only when the URL itself is the content.
- Code: reformat to ≤30 columns before printing; the tape will not wrap or
  warn, it clips.
- Figures: PNG/JPEG next to the .md, pre-scaled to ≤576 px wide; transparent
  ground prints as paper. High-contrast line art dithers best.
- Canonical specimens live in `crates/tm20-md/fixtures/` (01-prose … 14-fga);
  print them all with `print md crates/tm20-md/fixtures`.

## Rhythm and paper — what a block costs

The engine sets vertical air by the *pair of neighbors*; compose with it,
not against it.

- **What kisses:** the rule right after a `#` masthead, and the rule right
  after a table — with the next table hanging from it. That is the receipt
  idiom: items, thin `---`, bold total row. Write the total as a header-only
  table (`| **total** | | **13.00** |` plus its delimiter row) — a
  header-only table prints as one bold line. A `##` head also sits directly
  on whatever follows it, prose or table.
- **What breathes one 8-dot module:** stacked `##` heads, sibling lists,
  quotes, and code blocks, and everything else meeting a rule.
- **A rule is cheaper than a blank.** A `---` costs 18 dots all-in; a
  paragraph gap costs 37. Say sections with rules, not with empty air.

Paper math, for budgeting a tape (8 dots ≈ 1 mm): body line 37 · note line
29 · masthead line 51 plus 51 of air under it · paragraph gap 37 · sibling
seam 8 · rule 18 · figure its height + 16 · the notes apparatus 13 plus 29
per note line. A receipt reads best under ~800 dots (10 cm) before the
printer adds its feed-and-cut tail. Lists spend ~7 modules of the measure on
the mark column (the dash, `10.`, and the checkbox share it); a quote spends
two. The fewer nested structures, the longer your lines.

## Model tape

This file lowers cleanly (verified) — 577 dots ≈ 72 mm of paper:

```markdown
# Fika

Order 88 -- window seat.

| item | qty | total |
| :--- | ---: | ---: |
| cortado | 1 | 4.00 |
| cardamom bun | 2 | 9.00 |

---

| **total** | | **13.00** |
| :--- | ---: | ---: |

Pay at [the counter](https://pay.example.com/88 "Order 88").[^r]

- [x] paid
- [ ] served

[^r]: Keep this tape.
```

Masthead; en dash from `--`; the receipt idiom — items, thin rule, a bold
total row hanging from it as a header-only table; money with fixed decimals
sharing a right edge; checkboxes; and the notes apparatus: `1. Order 88` with
its URL captioned beneath, `2.` the footnote — all at 8 pt under a short rule.
