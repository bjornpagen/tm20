# Snapshot suite: visual regression for the tm20 typesetter

This document is a complete work order. It specifies a pixel-exact visual
regression suite for `tm20-md` + `tm20-set`, plus a font migration that must
land first. Execute it as written. Where the document says **observe**, the
behavior is intentionally unspecified: render it, record what happens in the
findings file, do not guess. If this document and the code disagree, the code
wins — note the discrepancy in findings rather than silently deviating.

Deliverables land in this repository. Do not publish any crate. Do not touch
`crates/tm20` (the protocol crate). Findings go to
`proposals/snap-suite/01-findings.md` (create it; numbered entries).

---

## 0. Ground rules

- Toolchain is pinned by `rust-toolchain.toml` (nightly-2026-08-15). The gate
  is `cargo clippy --workspace --all-targets -- -D warnings` clean and
  `cargo test --workspace` green. Run both before every commit.
- **Zero new dependencies.** `image` is already a dev-dependency of `tm20-md`
  (you may enable its `jpeg` feature for the asset generator; that is not a
  new dependency). No `insta`, no `pixelmatch`, no hash crates, no
  `libtest-mimic`.
- No `unsafe`. Edition 2024. Workspace lints apply.
- **No comparison thresholds.** The paint pipeline is deterministic
  (unhinted fontdue at integer ppem, harfrust shaping, fixed Floyd–Steinberg,
  no clocks, no hash-ordered output). Any pixel difference is a real change.
  Tolerance knobs would only hide regressions; comparison is exact equality.
- House test style (see `crates/tm20-set/tests/README.md`): each test is one
  fact. Corpus files follow the same rule: one concern per file, minimal
  context, no boilerplate.
- Commits are single-concern, imperative sentence with a period, saying why
  (match `git log --oneline` style, e.g. "Accept glyf OpenType so Helvetica
  can be the house face."). Purge, harness, each corpus area, and each bug
  fix are separate commits. A bug fix commits together with its minimal-repro
  corpus file and updated golden.
- Corpus markdown must contain **no raw HTML** — including `<!-- comments -->`
  — because HTML is a hard error in this dialect. Intent lives in the
  filename and in this document, never inside the file.

## 1. System under test — orientation crib

Pipeline: markdown → comrak AST → `tm20_md::sheet()` → `Sheet` of `Frame`s →
`tm20_set::compose(&sheet, &faces)` → one `tm20::Graphics` (full-height 576-dot
1-bit raster) → (`lower()` splits into ≤910-dot bands for the printer — not
snapshotted, see §3.6).

Key sources: `crates/tm20-md/src/lower.rs` (markdown→Frame mapping),
`crates/tm20-set/src/compose.rs` (paint + rhythm), `frame.rs` (Frame types,
`Figure::from_image`), `size.rs` (sizes), `preview.rs` (2× screen preview —
not used for goldens), `crates/tm20-md/tests/paper.rs` (existing fixture
tests), `crates/tm20-md/fixtures/` (style specimens), `audit/` (per-spec-
section decision records; statuses keep / never / unproven / gap).

Constants: tape 576 dots wide at 203 dpi (8 dots ≈ 1 mm). Body 11 pt = 31 ppem
(Plus2 slug 37 dots); notes 8 pt = 23 ppem (slug 29); masthead 18 pt display =
51 dots solid. `GRID` = 8 dots (code/quote hang, gutters). Bands cap at 910
dots.

Dialect facts corpus authors must know (verified against `lower.rs`):

| Construct | Renders as |
| --- | --- |
| `# H1` (every one) | 18 pt Helvetica display Mark, flush left |
| `##`–`######` | all identical 11 pt Helvetica Bold Head |
| heading interiors | flattened to plain text; links lose notes; math inside is a hard error |
| paragraph | 11 pt Helvetica Roman, ragged right, ~36–42 chars/line |
| `*em*` / `**strong**` / `***both***` | Oblique / Bold / BoldOblique cuts |
| `` `code` `` / fences / indented code | Menlo (after §2); blocks hang 8 dots and **never wrap** — long lines clip at the tape edge |
| `---` | full-measure rule, 2 dots thick |
| lists | `-`/`*`/`+` all print a dash; ordered honors start + `.`/`)`; `- [ ]`/`- [x]` checkboxes; tight/loose honored; nest cap 3 |
| `> quote` | hangs 8 dots per level, nest cap 3 |
| tables | 2–3 columns only; `---:` = right, `:---:` and `:---` = left; header row Bold |
| `[text](url)` | italic text + superscript number; URL (+ optional `"title"`) as an 8 pt numbered endnote under a short rule; duplicate URLs share a number; `url == text` gets no note; `mailto:` stripped in the note |
| autolinks (`<…>`, bare GFM) | italic, no note |
| `[^x]` | endnote in the same number sequence; definitions render at 8 pt; unused definitions dropped |
| `\(…\)` / `\[…\]` | RaTeX-typeset math, inline / display; `$` is never math |
| image alone in a paragraph | PNG/JPEG decoded, shrunk if wider than 576 (never upscaled), centered, Floyd–Steinberg dithered; alt text never prints; dest resolves relative to the .md |
| smart punctuation | on: quotes curl, `--` → en dash, `---` → em dash |
| `~~x~~` | literal tildes (extension off on purpose) |

Hard errors (exact `Display` strings from `crates/tm20-md/src/error.rs`):
"raw HTML is not representable" · "a paragraph cannot mix text and an image" ·
"could not load figure" · "could not typeset math" · "quote or list nested
more than three deep" · "table must have two or three columns" · "footnote
has no definition". Note `tm20_set::Error::Image` (decode failure) maps to
the *load* string, not a distinct decode string — verify on the machine
before writing expectations.

Render any corpus file for inspection without a printer:

    cargo run --bin tm20-set -- --dry --png target/corpus-preview print md <path.md>

## 2. Phase 0a — purge Commit Mono, adopt Menlo (lands first)

`Cut::Mono` moves from user-installed `CommitMono-400-Regular.otf` to system
`/System/Library/Fonts/Menlo.ttc` (PostScript name `Menlo-Regular`; select by
scanning collection indices exactly like the existing Helvetica loop). After
this, every face is a system face and `~/Library/Fonts` is dead. Menlo over
the alternatives: static public TTC, four real cuts, stable PostScript names;
SF Mono (`SFNSMono.ttf`) is Apple-private with dot-prefixed hidden names;
Monaco has one cut.

Touch exactly these (12 references, 8 files — re-grep `-iE "commit ?mono"` to
confirm none survive):

1. `crates/tm20-cli/src/kit.rs` — delete `commit_mono()` and `fonts_dir()`;
   load Menlo-Regular from `Menlo.ttc` and `table.set_text(Cut::Mono, …)`.
2. `crates/tm20-cli/src/main.rs` — usage string: faces are Helvetica and
   Menlo from /System/Library/Fonts.
3. `crates/tm20-set/tests/common/mod.rs` — same swap; fix the module doc line.
4. `crates/tm20-md/tests/paper.rs` — `load_mono()` reads Menlo.ttc by index.
5. `crates/tm20-set/tests/algebra.rs` — update the one prose mention.
6. `crates/tm20-set/tests/README.md` — fonts paragraph.
7. `crates/tm20-md/fixtures/04-code.md` — "Fenced, hung, Menlo."
8. `.claude/skills/tm20/SKILL.md` — four mentions **plus re-measure the mono
   line budget**: render a code ruler (`123456789A123456789B…` to 50 chars)
   through the CLI, read the PNG, count the last visible column, and update
   the skill's "~34 chars" (expect ≈30–31 with Menlo) and its prerequisites
   line.

Verify: render `crates/tm20-md/fixtures/04-code.md`, confirm mono renders and
the overflow line clips; run the gate. One commit.

## 3. Phase 0b — the harness

### 3.1 Layout

    crates/tm20-md/tests/
      snap.rs               harness (new)
      common/mod.rs         face table + font digest, shared by snap.rs and paper.rs (new; refactor paper.rs to use it)
      faces.lock            font digests (committed)
      corpus/*.md           the corpus (committed)
      corpus/assets/*       images corpus files reference (committed, generated — §3.7)
      goldens/<stem>.png    one golden per corpus file (committed)
      reject/*.md           markdown that must fail (committed)
      reject/expect.txt     stem = exact error Display string (committed)

Add to `crates/tm20-md/Cargo.toml`:
`exclude = ["tests/corpus", "tests/goldens", "tests/reject"]` so the
published crate stays small. `snap.rs` returns early (with an eprintln) when
`tests/corpus` is absent, so `cargo test` still passes from a published
.crate.

### 3.2 Goldens

One artifact per corpus file: the composed raster losslessly wrapped as a
grayscale PNG at 1× (576 px wide). Encode from `Graphics` bits (ink = 0x00,
paper = 0xFF) with the `image` dev-dep. Prefer bit depth 1
(`ExtendedColorType::L1`); if the encoder refuses L1, L8 is acceptable — the
compare never reads encoder output byte-for-byte. The PNG *is* the golden:
decoding returns the exact raster, so there is no separate hash or manifest
to drift. Do not use `preview_png` (2× is doubled bytes for zero
information).

### 3.3 Compare

For each corpus file, sorted by name: `sheet(src, Measure::TAPE, load)` →
`compose` → raster. Decode the golden to luma, threshold at 128, compare
dimensions first, then every pixel via `tm20::graphics::{is_black,
width_bytes}` against the fresh raster. Exact match or fail. The image loader
is `|d| image_bytes(md_parent, d)` — asset paths in corpus files are relative
(`assets/foo.png`).

Collect **all** failures across the corpus, then panic once with a table:
stem, kind (missing golden / dims changed / N pixels differ), diff bounding
box, height delta.

### 3.4 Failure artifacts (the Playwright triplet, minus thresholds)

On any mismatch write to `target/snap/`: `<stem>.expected.png`,
`<stem>.actual.png`, `<stem>.diff.png`. Diff is RGB: paper white, matching
ink #666666, ink only in actual (added) #DD0000, ink only in expected
(removed) #0066CC. Playwright needs YIQ thresholds and maxDiffPixels because
browser rendering is noisy; this raster is noiseless 1-bit, so the threshold
is zero by design.

### 3.5 Bless and lock

- `TM20_SNAP=bless cargo test -p tm20-md --test snap` rewrites goldens for
  missing/mismatching corpus files, rewrites `faces.lock`, and reports what
  it wrote. Anything else (unset or unknown value) = compare mode.
- `faces.lock`: two lines, `helvetica <hex>` and `menlo <hex>` — FNV-1a 64
  over the font file bytes (offset basis 0xcbf29ce484222325, prime
  0x100000001b3; hand-rolled ~10 lines, no dependency; this detects drift,
  it is not security). Every snap test first checks the lock and fails with
  "font drift — inspect and re-bless" when digests differ. A macOS update
  thereby becomes a conscious re-bless, never a mystery diff.

### 3.6 Tests inside snap.rs

- `corpus_matches_goldens` — §3.3.
- `rejects_reject` — every `reject/*.md` must return `Err` from `sheet()`
  whose `Display` string equals its `expect.txt` line. A reject file that
  parses cleanly is a failure. Missing/extra expect lines are failures.
- `faces_are_locked` — standalone lock check with a readable message.
- `compose_is_deterministic` — compose one seed file twice, assert identical
  rasters (cheap insurance for the exactness premise).

Band splitting is deliberately **not** snapshotted: `lower()` is a pure row
partition of the composed raster and `paper.rs::fga_lesson_splits_into_min_
payloads` already pins the partition property. One tall corpus file (§4,
`set-tall-a`) keeps the >910-dot path exercised end to end.

### 3.7 Asset generator

`#[test] #[ignore] fn write_corpus_assets()` in snap.rs, run once via
`cargo test -p tm20-md --test snap -- --ignored write_corpus_assets`, then
commit the binaries. Exact formulas (deterministic, so regeneration is
byte-identical):

| Asset | Spec |
| --- | --- |
| `sq60.png` | 60×60, black 4-px border, white interior |
| `w575.png` `w576.png` `w577.png` | width×24, solid black |
| `vline.png` | 1×1200 black |
| `hline.png` | 576×1 black |
| `ramp.png` | 256×64 horizontal gradient, luma = x |
| `alpha.png` | 64×64 RGBA, opaque black circle radius 24 on fully transparent ground (**observe** how alpha lumafies) |
| `indexed.png` | 64×64 paletted checkerboard, 8-px cells |
| `gray.png` | 64×64 Luma8 vertical gradient, luma = y·4 (offset −1 at y=64… use min(255, y·4)) |
| `photo.jpg` | 128×96 RGB, luma = (x+y)·255/222, JPEG quality 80 |
| `garbage.png` | 64 bytes of 0xA5 (not a PNG; reject fodder) |

## 4. The corpus

Naming: `<area>-<letter>-<slug>.md`, flat in `tests/corpus/`. One concern per
file. Keep files short — a golden should be one screen. If a listed file
turns out to mix facts, split it (`-a1`, `-a2`) rather than growing it.
Where a row says **observe**, render first, describe the result in findings,
then bless whatever is correct-by-adjudication (§6).

### 4.1 CommonMark — characters, blocks (mirrors `audit/cm-*`)

| File | Fact under test |
| --- | --- |
| cm-2.1-a-line-endings | CRLF and lone-CR sources render identically to LF |
| cm-2.2-a-tabs-code | tab-indented code block; tabs inside fences survive as columns |
| cm-2.2-b-tabs-lists | tab after list marker; tabbed continuation indentation |
| cm-2.3-a-insecure | U+0000 becomes U+FFFD (**observe** the glyph) |
| cm-2.4-a-escapes | backslash before each ASCII punct prints the literal; before letters stays a backslash |
| cm-2.4-b-escape-contexts | escapes inert inside code spans and fenced code |
| cm-2.5-a-entities | `&amp; &lt; &#35; &#xA9; &copy;` render as glyphs; `&nbsp;` (**observe**); `&foo;` stays literal |
| cm-3.1-a-precedence | block structure beats inline: list marker vs backtick spanning items |
| cm-3.2-a-containers | quote containing a list whose item continues on a lazy line |
| cm-4.1-a-break-markers | `---` `***` `___`, spaced `- - -`, 4+ chars: all the same 2-dot rule |
| cm-4.1-b-break-vs-setext | `---` directly under text = setext H2 (bold head); after a blank = rule |
| cm-4.2-a-atx-levels | `#`–`######`: one masthead, five identical bold heads (no ladder) |
| cm-4.2-b-atx-forms | closing hashes stripped; `\#` literal; empty heading; interior space collapse |
| cm-4.2-c-atx-flatten | `#` with em/strong/code/link: all flatten to plain masthead text, no note |
| cm-4.3-a-setext | `===` masthead over two source lines (soft break → space); `---` bold head |
| cm-4.4-a-indented-code | 4/5/8-space indents (extra indent preserved), blank interior line |
| cm-4.5-a-fences | backtick and tilde fences; empty block; unclosed fence at EOF |
| cm-4.5-b-fence-info | info strings (`rust`, `text`, gibberish) change nothing |
| cm-4.5-c-fence-content | shorter fence chars inside a longer fence; indented fence strip |
| cm-4.5-d-fence-in-contexts | fenced code inside a list item and inside a quote (hang composition) |
| cm-4.7-a-ref-links | full/collapsed/shortcut reference links; case-folded labels; def title reaches the note |
| cm-4.7-b-ref-edges | duplicate defs (first wins); unused def invisible; doc of only defs (**observe**) |
| cm-4.8-a-paragraphs | up-to-3 leading spaces stripped; 4 becomes code |
| cm-4.9-a-blank-lines | 1 vs 5 blank lines identical; leading/trailing document blanks invisible |

### 4.2 CommonMark — quotes and lists

| File | Fact under test |
| --- | --- |
| cm-5.1-a-quote-basic | one-liner; multi-paragraph quote; bare `>` line inside |
| cm-5.1-b-quote-lazy | lazy continuation lines bind to the quote |
| cm-5.1-c-quote-nested | 2- and 3-deep nesting; hang accumulates 8 dots per level |
| cm-5.1-d-quote-contents | quote holding a head, a list, fenced code, and a rule |
| cm-5.2-a-item-indent | marker-plus-1/2/3-space content columns; interior indented code inside an item |
| cm-5.2-b-item-blocks | item with two paragraphs; item starting with a fence; item holding a quote |
| cm-5.2-c-item-empty | empty items between full ones (**observe**) |
| cm-5.2-d-item-heading | `##` inside a list item (**observe**) |
| cm-5.3-a-markers | `-` `*` `+` all print dashes; marker switch starts a new list (**observe** the seam) |
| cm-5.3-b-ordered-start | starts 1, 0, 7, 999999999; `)` delimiter; hang fits the widest marker |
| cm-5.3-c-tight-loose | same items tight vs loose; one interior blank loosens the whole list |
| cm-5.3-d-interrupt | only `1.` interrupts a paragraph; `7.` does not |
| cm-5.3-e-nested-mixed | ul→ol→ul at the 3-deep cap, alternating markers |
| cm-5.3-f-runover | long item text wraps clear of the mark column (dash and 3-digit decimal) |

### 4.3 CommonMark — inlines

| File | Fact under test |
| --- | --- |
| cm-6.1-a-code-spans | backtick-run counting; one-space strip rule; backticks inside a span |
| cm-6.1-b-span-breaks | line ending inside a code span becomes a space |
| cm-6.2-a-flanking | `*a**b*`, `**a *b* c**`, spec left/right-flank torture picks |
| cm-6.2-b-intraword | `mid*word*` italicizes; `mid_word_` does not |
| cm-6.2-c-mixed-delims | `*_both_*`, `_*crossed*_`, `***strong em***` split order |
| cm-6.2-d-adjacent-runs | `*a**b**c*` — adjacent runs; span merging produces fewest cuts |
| cm-6.2-e-punct-flanks | emphasis against quotes/parens/smart punctuation |
| cm-6.3-a-inline-links | dest with `%20`; titles in `"` `'` `(` styles; angle-bracket dest; empty text; empty dest |
| cm-6.3-b-link-nesting | em inside link; strong inside link; code inside link (link text is already italic) |
| cm-6.3-c-link-notes | same URL twice = one note number; `url == text` = no note; mailto strip; title line in the note |
| cm-6.3-d-brackets | literal `[x]`, escaped `\[x\]`, shortcut-lookalike with no definition stays literal |
| cm-6.4-a-images | figure alone; alt with markup never prints; reference-style image |
| cm-6.5-a-autolinks | `<https://a.b/c>` and `<mailto:x@y.z>` italic without notes |
| cm-6.5-b-gfm-autolink | bare `www.…` and `https://…`; trailing `.,;:)` excluded; bare email |
| cm-6.7-a-hard-breaks | two-space and backslash breaks; inside emphasis; at paragraph end (inert) |
| cm-6.8-a-soft-breaks | soft break = single space, always |
| cm-6.9-a-unicode | é as NFC vs NFD (**observe** — shaping may differ), fi/fl ligatures via `liga`, ¶ • … |
| cm-6.9-b-scripts | Greek and Cyrillic lines (Helvetica covers); one CJK and one Arabic word (**observe** tofu/omission) |
| cm-6.9-c-tofu | U+FFFD, an unassigned codepoint, an emoji (**observe**) |

### 4.4 Extensions

| File | Fact under test |
| --- | --- |
| ext-table-a-two-col | label/value pairs, start/end alignment |
| ext-table-b-three-col | alignment combinations across L/L/R, L/R/R, R/R/R rows of one and several tables |
| ext-table-c-squeeze | preferred widths just over the measure — the squeeze path wraps a column |
| ext-table-d-overflow | minimum widths exceed the measure — the overflow/stacked path (**observe**) |
| ext-table-e-cell-content | bold, italic, code, links-with-notes inside cells; header bolding overrides |
| ext-table-f-pipes | `\|` escaped in cells; `` `a|b` `` code span with a pipe |
| ext-table-g-degenerate | empty cells; header-only table (**observe**) |
| ext-table-h-numeric | price columns right-aligned; decimal alignment behavior (**observe** DecimalDelim) |
| ext-task-a-basic | checked/unchecked boxes, tight; box hangs on the grid |
| ext-task-b-nested-loose | nested tasks; loose task list |
| ext-foot-a-basic | ref + def; unused definition never prints |
| ext-foot-b-multiblock | one def holding paragraph + list + code, all at 8 pt |
| ext-foot-c-order | numbering interleaves with link notes by first appearance |
| ext-foot-d-in-cell | footnote ref inside a table cell |
| ext-foot-e-in-quote | footnote ref inside a quote; def outside |
| ext-math-a-inline | short inline; tall `\frac` inline (**observe** line leading) |
| ext-math-b-display | narrow display centered?; display near measure width (**observe**) |
| ext-math-c-contexts | math in a list item and in a quote |
| ext-math-d-in-note | math inside a footnote definition (8 pt math) |
| ext-math-e-zoo | `\sqrt` `\sum` limits, Greek, operators, a small matrix if RaTeX takes it |
| ext-math-f-dollars | `$4.50` and `\(x\)` in one paragraph — dollars stay currency |
| ext-smart-a-quotes | nested "double 'single' double"; apostrophes; primes |
| ext-smart-b-dashes | `--` en, `---` em (inline), `...` ellipsis; en dash in ranges |
| ext-never-a-strike | `~~x~~` prints tildes |
| ext-never-b-autolink-off-cases | GFM oddities that must stay literal (partial `www`, `ftp://`) (**observe**) |

### 4.5 Typesetter stress (`set-*`) — the corner-case net

Every `set-pair-*` file *starts* with its leading frame type, which doubles as
the doc-starts-with-X edge case for all ten frame types.

| File | Fact under test |
| --- | --- |
| set-wrap-a-long-url | 100-char unbreakable URL token in prose (**observe**: clip, overflow, or split) |
| set-wrap-b-long-word | 200-char word at 11 pt and again inside a footnote at 8 pt (**observe**) |
| set-wrap-c-cut-boundaries | bold/italic flips landing exactly at the wrap point |
| set-wrap-d-note-at-margin | noted word at line end — superscript pull-back path needs a witness |
| set-wrap-e-full-measure | lines that exactly fill the measure (fox-style sentences) |
| set-pair-a-after-text … set-pair-j-after-rule | ten files; each opens with one frame type (text, head, mark, list, cols, quote, code, figure, math, rule) followed by every other type in sequence — the complete rhythm-pair matrix from `compose::extra()` |
| set-edge-a-empty | empty file (**observe**: empty tape? error? panic = finding) |
| set-edge-b-blank-only | only blank lines (**observe**) |
| set-edge-c-ends-rule | document ending on a rule |
| set-edge-d-ends-figure | figure as the final frame |
| set-edge-e-only-figure | a figure and nothing else |
| set-edge-f-notes-after-figure | noted link earlier, figure last — notes apparatus follows the figure |
| set-notes-a-many | 12+ notes: two-digit numbers, alignment of the note column |
| set-notes-b-long-url | 120-char URL inside a note at 8 pt (**observe** wrap vs clip) |
| set-notes-c-title-url | long title plus long URL in one note |
| set-fig-a-native | `sq60.png` at native size, centered |
| set-fig-b-measure-edge | `w575` `w576` `w577` in three paragraphs: near-full, full-bleed, shrunk-by-one |
| set-fig-c-extreme-aspect | `vline.png` (1×1200) and `hline.png` (576×1) |
| set-fig-d-dither | `ramp.png` and `gray.png` — locks the Floyd–Steinberg pattern |
| set-fig-e-modes | `alpha.png`, `indexed.png` (**observe** alpha handling) |
| set-fig-f-jpeg | `photo.jpg` decode + dither |
| set-nest-a-quote-cap | 3-deep quote with real sentence widths |
| set-nest-b-list-marker-width | 3-deep ordered list with `999.` markers at depth |
| set-nest-c-hang-pileup | quote > list > fenced code — hangs stack, usable measure gets small |
| set-cols-a-natural | all-start table renders at natural width with 8-dot gutters |
| set-cols-b-end-hang | end-aligned columns hang on the tape edge |
| set-tall-a-band-cross | ~2500-dot document (repeated paragraphs, rules, a figure) — exercises the >910 path end to end |

### 4.6 Synthetic documents (`doc-*`) — interaction at scale

| File | Contents |
| --- | --- |
| doc-a-receipt | masthead, en-dash prose, 3-col priced table, rule, tasks, noted link, footnote |
| doc-b-reading | essay: masthead, sections, display+inline math, footnotes, a quote — ≥2 bands tall |
| doc-c-changelog | heads, tight lists with code spans and autolinks |
| doc-d-spec-sheet | tables + figures + rules interleaved |

### 4.7 Reject corpus (`reject/*.md` + `expect.txt`)

Verify every string by running before committing `expect.txt`; record any
string that differs from this table in findings.

| File | Content sketch | Expected Display |
| --- | --- | --- |
| rej-html-a-block | `<div>x</div>` | raw HTML is not representable |
| rej-html-b-inline | `a <b>b</b>` | raw HTML is not representable |
| rej-html-c-comment | `<!-- x -->` | raw HTML is not representable |
| rej-html-d-bare-tag | `a <foo> b` | raw HTML is not representable |
| rej-table-a-one-col | single-column pipe table | table must have two or three columns |
| rej-table-b-four-col | four columns | table must have two or three columns |
| rej-table-c-ragged | 3-col header, 2-cell row | table must have two or three columns |
| rej-image-a-mixed | text and `![x](assets/sq60.png)` in one paragraph | a paragraph cannot mix text and an image |
| rej-image-b-remote | `![x](https://e.com/a.png)` | could not load figure |
| rej-image-c-missing | `![x](assets/absent.png)` | could not load figure |
| rej-image-d-garbage | `![x](assets/garbage.png)` | could not load figure (verify — decode error path) |
| rej-math-a-heading | `# \(x\)` | could not typeset math |
| rej-math-b-bad-latex | `\(\frac{\)` | could not typeset math (verify) |
| rej-nest-a-quote-4 | 4-deep quote | quote or list nested more than three deep |
| rej-nest-b-list-4 | 4-deep list | quote or list nested more than three deep |
| rej-foot-a-undefined | `x[^n]`, no def | footnote has no definition |

## 5. Work packets (fanout map)

P0a and P0b are serial and land first. P1–P4 are independently authorable in
parallel once P0b exists; authors produce **markdown files only** — never
goldens (blessing is centralized in P5). Authors verify their own files with
the CLI preview command (§1); reject authors verify the error string instead.

| Packet | Contents | Depends on |
| --- | --- | --- |
| P0a | Menlo purge (§2) | — |
| P0b | harness + faces.lock + asset generator + 10 seed corpus files (pick one from each of §4.1–4.6 areas) + Cargo.toml exclude | P0a |
| P1 | §4.1 + §4.2 + §4.3 (CommonMark corpus) | P0b |
| P2 | §4.4 (extensions) | P0b |
| P3 | §4.5 + §4.6 (typesetter stress + documents) | P0b |
| P4 | §4.7 (reject corpus + expect.txt) | P0b |
| P5 | review + adjudication + fixes + final bless + docs (§6, §7) | P1–P4 |

## 6. Review and bless protocol (P5) — the first bless is a bug hunt

1. `TM20_SNAP=bless` over the full corpus; **do not commit goldens yet**.
2. Review every golden PNG. For each suspect, write a numbered finding in
   `proposals/snap-suite/01-findings.md`: corpus stem, what looks wrong, the
   suspected frame path (`compose.rs` fn). **observe**-marked files always
   get an entry describing actual behavior.
3. Adjudicate each finding: bug / intended / accepted-ugly. Bugs get fixed —
   one commit per fix, carrying its minimal-repro corpus file and refreshed
   golden.
4. Re-bless clean, commit corpus + goldens + faces.lock as one commit.

## 7. Documentation and audit integration (P5)

- Write `crates/tm20-md/tests/README.md`: what the suite is, the exact bless
  command, the triplet artifact paths, the faces.lock contract, how to add a
  corpus file (author → preview → findings if odd → bless → commit).
- Every `audit/*.md` gains a `Corpus:` line listing its corpus stems. The
  files currently marked **unproven** (grep `-l unproven audit/`) flip to
  **keep** where a golden now owns the fact; note flips in findings.
- Confirm `cargo package -p tm20-md --list` shows no corpus/golden/reject
  paths.

## 8. Acceptance

- Gate green: clippy `-D warnings` + `cargo test --workspace` (includes the
  snap suite) on the pinned toolchain.
- No `commit ?mono` match anywhere in the repository.
- ~150+ corpus goldens committed; all 16 rejects pinned to exact strings;
  faces.lock present; `TM20_SNAP=bless` idempotent (second run writes
  nothing).
- Every **observe** row has a findings entry; every finding is adjudicated.
- Published-crate hygiene: excludes verified (§7).

## 9. Out of scope

No CI. No vendored fonts. No comparison thresholds or maxDiffPixels-style
knobs. No new dependencies. No 2× goldens, no band goldens, no sizes
manifest. No changes to `crates/tm20`. No publishing (that happens after this
suite is green). No page-mode/NV/protocol features. Do not reorganize
`fixtures/` — `paper.rs` owns it; the corpus is additive.
