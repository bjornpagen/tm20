# Typography review — snap-suite tapes

Visual + typographic pass over every composed PNG, read as a 576-dot
1-bit Helvetica + Menlo receipt. Doctrine is Massimo Vignelli (*The
Vignelli Canon*) and Jan Tschichold (*Typographische Gestaltung* / *The
Form of the Book* / *Die neue Typographie*). Engine law (GRID = 8,
three sizes, two faces, no decoration) is the brief, not a suggestion.

This file does not replace `01-findings*.md`. It confirms those
observations from the rasters and adds defects a human would notice on
the tape. Corpus intent is left alone.

---

## 1. Method

Inspected **123 / 123** goldens in `crates/tm20-md/tests/goldens/`
(1×, 576 px wide — the bless raster). Also opened the 2× CLI previews
when the 1× was too thin to judge hang, clip, or collision:

| Dir | PNGs | Role |
| --- | ---: | --- |
| `crates/tm20-md/tests/goldens/` | 123 | Bless output; every stem opened |
| `target/corpus-preview-p1/` | 58 | 2× of the CommonMark stems |
| `target/corpus-preview-p2/` | 25 | 2× of the extension stems |
| `target/corpus-preview-p3/` | 38 | 2× of the set-/doc- stems |
| `target/corpus-preview-p4/` | 5 | Reject experiments (not goldens) |
| `target/corpus-preview/` | 17 | Fixture / ruler / seed previews |

**Unique stems: 135.** Every golden stem was opened. Extra stems without
a golden (`04-code*`, `mono-ruler*`, five `rej-*` experiments) were
opened as well.

Ink-edge measurement (first/last black column, rule spans, empty
height) was used to check what the eye misses at 1×: 8-dot quote hang,
568-dot in-quote rules, 144-dot note rules, right-edge clip.

**Skipped:** none of the goldens. `04-code-fence.png` and
`mono-ruler-left.png` were not opened as separate files (same fixture
family as `04-code.png` / `mono-ruler.png` / `04-code-hang.png`).
Nothing failed to decode.

---

## 2. Doctrine on this tape

The engine already chose a Vignelli brief: two faces, three sizes
(18 display / 11 body / 8 notes), one GRID, flush-left rag, no
blockquote bar, no table rules, no size ladder under `#`. That is
correct. Do not “enrich” it.

What Vignelli and Tschichold still demand *here*:

- **GRID is architecture.** An 8-dot hang is the quantum. It must be
  real (it is) but it is optically quiet on 576 dots (~1.4 mm). Quotes
  read as body unless the measure also narrows. That is acceptable
  reduction only if nothing else (a rule, a list mark) then lies about
  the left edge.
- **Rules are either full-measure or a named short.** `NOTE_RULE` =
  18×GRID = 144 is named and on-grid — keep it. A rule that is 568
  because a quote stole 8 dots is “almost-full by accident.”
- **A word space is a word space.** Punctuation after a styled span
  (link, code, autolink, note mark) must not grow a hole. Tschichold’s
  even color dies first on this tape at those seams.
- **Marks hang; text is a column.** Dash, `1.`, `12.`, checkbox share
  one content edge. Wrap clears that edge. Empty items still occupy
  the mark column — they must not look like a stray tick.
- **Unbreakable tokens may clip, but the clip must be honest.** A
  mid-glyph chop at x=575 is better than a collision. Overflow that
  paints into the next column is a defect, not a style.
- **Figures and display math may center.** Everything else stays left.
- **Tofu is honest** for CJK / emoji / unassigned. Tofu for TAB or a
  dropped NUL is not.
- **Masthead / bold head / body / notes** is the whole size set. Do
  not invent a fourth.

---

## 3. Numbered issues

Known snap-suite findings are restated only where the PNG adds a
typographic reading. New issues start at 16.

### 1. Table overflow paints colliding glyphs

- **Stems:** `ext-table-d-overflow` (also the tight L/R/R and R/R/R
  rows of `ext-table-b-three-col`, `set-cols-b-end-hang` as a near-miss)
- **PNG:** Two slugs. Header `ABCDEFGHIJKLMN…` and body
  `abcdefghijklmnopqrst…` run past their cell boxes; later glyphs
  overprint earlier ones and both tape edges. Density ~0.31 — the
  darkest tape in the suite. `set-cols-b-end-hang` keeps the right
  edge but the gutter between `12` and `48.00` is a few dots.
- **Principle:** Tschichold — crashing glyphs / even color. Engine
  finding (P2 §1).
- **Suspect:** `paint_grid` → `paint_line` / `blit` (`compose.rs`);
  `cols.rs::overflow` shrinks flex boxes and never clips ink.
- **Severity:** blocker
- **Fix:** In `paint_line` (or a cell-scoped blit), drop or clip
  glyphs whose ink exceeds the cell `origin+width`. Overflow layout
  may stay one-row; it must not paint into the neighbor.

### 2. `paint_line` inserts a word space between every piece

- **Stems:** `cm-6.5-a-autolinks`, `cm-6.5-b-gfm-autolink`,
  `cm-6.1-a-code-spans`, `doc-a-receipt`, `set-wrap-d-note-at-margin`,
  `set-edge-f-notes-after-figure`, `ext-math-d-in-note`
- **PNG:** Source `See <https://a.b/c> and <mailto:x@y.z>.` has no
  space before the period. The tape shows `…y.z .` — a full word
  space, then the stop. Same hole after a noted word:
  `the register¹ .²`, `cited¹ .`, `See the site¹ .`. Autolink
  trailers `.,;:)` in `cm-6.5-b` sit one space off the URL. In
  `cm-6.1-a` a comma starts the second line (`, and tight .`).
- **Principle:** Tschichold — holes in the color; hanging / leading
  punctuation that breaks the left edge. Vignelli — an element (the
  space) that does no work.
- **Suspect:** `paint_line` (and `line_width`) always add
  `face.shape(" ")` between pieces. `wrap_plan` splits spans on
  spaces, so a roman period after an italic autolink is a new piece
  and gets a space it never had in the source.
- **Severity:** blocker (the model receipt is wrong)
- **Fix:** Carry a `lead_space: bool` on `Piece` from `wrap_plan`
  (true only when the source had a space before this word). 
  `paint_line` / `line_width` add the word space only then.

### 3. Unbreakable URL / word clips mid-glyph at the tape edge

- **Stems:** `set-wrap-a-long-url`, `set-wrap-b-long-word`,
  `set-notes-b-long-url`, `set-notes-c-title-url` (URL line),
  `04-code-hang` / fixture overflow
- **PNG:** A 100-char italic URL parks on its own line and chops at
  x=575 (`right_gap` 0). The last visible `x` is cut. Body and 8 pt
  `w`×200 do the same. Note URLs do not wrap; title lines in
  `set-notes-c` do wrap and clear the mark — only the destination
  token clips.
- **Principle:** Tschichold — measure. Engine finding (P3 §1, §2, §5).
  Honest clip beats collision; silent loss of the destination is still
  a tape defect.
- **Suspect:** `wrap_chunk_plan` (one box wider than the measure stays
  on one line) + `blit` (x ≥ width dropped).
- **Severity:** blocker (information loss) — clip is the current
  contract for code; notes/prose URLs should wrap on punctuation
  (`/`, `?`, `&`, `.`) or hyphenate, not vanish.
- **Fix:** In `wrap_chunk_plan`, if a single piece exceeds `measure`,
  split on URI punctuation (or every N dots) before accepting the
  overflow line. Leave fenced `paint_code` as clip — that is the
  dialect.

### 4. `alpha.png` lumafies to a solid black square

- **Stems:** `set-fig-e-modes` (upper figure), asset `alpha.png`
- **PNG:** Centered 64×64 solid black. Every row 256–319 is fully
  inked. The r=24 circle is gone. The indexed checkerboard under it
  is correct (8 px cells).
- **Principle:** Engine finding (root §3, P3 §6). Vignelli — the
  figure does the wrong work.
- **Suspect:** `Figure::from_image` → `image::to_luma8` drops alpha;
  `(0,0,0,0)` and `(0,0,0,255)` both become luma 0.
- **Severity:** blocker (wrong reject of transparent ground)
- **Fix:** Composite onto white (or paper) before luma:
  `out = rgb * a + 255 * (1-a)`, then dither.

### 5. Fence TAB paints Menlo `.notdef` tofu

- **Stems:** `cm-2.2-a-tabs-code` (2× preview confirms `col▯umn`)
- **PNG:** Line 1 `tab indented` is flush to the code hang (tab used
  as indent, consumed). Line 2 has a hollow rectangle between `l`
  and `u` — U+0009 survived inside the fence and Menlo has no tab
  glyph.
- **Principle:** Tschichold — a hole that is not a character. Engine
  finding (P1 §2). Tofu for CJK is honest; tofu for TAB is not.
- **Suspect:** `paint_code` / `TextFace::shape` — no TAB → column
  expansion.
- **Severity:** dialect / color (visible on any fenced tab)
- **Fix:** In `paint_code` (or the mono shaper), expand TAB to N
  spaces toward the next 4-column stop before `shape`. Do not emit
  U+0009.

### 6. U+0000 paints no FFFD ink

- **Stems:** `cm-2.3-a-insecure`
- **PNG:** `beforeafter` as one word. ~5-dot empty advance, no
  replacement box. Contrast `cm-6.9-c-tofu`, where an explicit U+FFFD
  is a hollow square.
- **Principle:** Engine finding (P1 §1). Tschichold — a collision of
  two words that should have been separated by a visible mark.
- **Suspect:** comrak strips NUL, or Helvetica draws FFFD with no
  ink. `paint_run`.
- **Severity:** dialect
- **Fix:** After parse, map leftover U+0000 → U+FFFD in the lower, or
  reject the file. Do not leave a blank advance.

### 7. Rule inside a quote is 568 dots — almost-full by accident

- **Stems:** `cm-5.1-d-quote-contents` (rule rows y=111–112: x=8…575,
  568 ink). Nested rules in `set-pair-*` that sit under a hung
  measure.
- **PNG:** Head, dash-item, hung `code`, then a 2-dot rule that
  starts at the quote hang and runs to the right tape edge. Eight
  dots of paper on the left; none on the right. Looks like a broken
  full-measure rule, not a quote-scoped rule.
- **Principle:** Tschichold — rules are full-measure or clearly
  motivated; never almost-full by accident. Vignelli — alignment is
  structural: if the quote voice is only 8 dots, a 568-dot rule
  shouts louder than the text.
- **Suspect:** `paint_rule` uses `x0` + `measure` of the quote
  (`paint_quote` subtracts GRID).
- **Severity:** align
- **Fix:** Either paint quote rules at tape x=0…576 (full-measure,
  ignoring hang) or paint a short named rule (e.g. remaining measure
  minus a GRID, or `NOTE_RULE`-class). Do not leave 568.

### 8. `extra(Hang, Hang) = 0` — list / quote / code seams are invisible

- **Stems:** `cm-5.3-a-markers`, `cm-5.2-b-item-blocks`,
  `set-pair-f-after-quote`, `set-pair-g-after-code`
- **PNG:** `-` `*` `+` all paint en-dashes. Three lists look like one
  tight list — no slug at the marker switch. Quote then code in the
  pair tapes sit on consecutive slugs with no extra air.
- **Principle:** Engine finding (P1 §8). Vignelli — white space as
  architecture between *kinds*. A new list is a new movement.
- **Suspect:** `compose.rs` `extra`:
  `(Rhythm::Prose | Rhythm::Hang, Rhythm::Hang) => 0`.
- **Severity:** rhythm
- **Fix:** Keep Hang→Hang = 0 *inside* one list (`paint_list` already
  owns item rhythm). Between sibling Hang *frames* (new list, or
  quote then code), return `next` (one GRID or the incoming slug).
  Distinguish “same list” from “new hang frame” — `extra` only sees
  rhythm, so either tag List vs Quote vs Code in `Rhythm` or apply
  the slug at `paint_list` entry when `cur.last == Hang`.

### 9. Empty list item is a mark-only tick

- **Stems:** `cm-5.2-c-item-empty`
- **PNG:** Height 88. After `– full` (dash at x=0, text at ~56),
  rows y=27–29 are ink only at x=0…16 (the en-dash), then ~21 empty
  rows, then `– also full`. The empty item is a stray dash with no
  text and a short slug.
- **Principle:** Engine finding (P1 §6). Tschichold — orphan of
  structure. Vignelli — an element that does no work.
- **Suspect:** `paint_list` still pushes `Pending::Glyph` for an
  empty `item.frames`.
- **Severity:** rhythm
- **Fix:** If `item.frames` is empty, still occupy one body slug so
  the dash sits on a full line (same as a one-word item), or drop
  the mark. A 3-dot-tall dash is the worse of both.

### 10. Empty / defs-only documents are an 8-dot blank slug

- **Stems:** `set-edge-a-empty`, `set-edge-b-blank-only`,
  `cm-4.7-b2-only-defs`
- **PNG:** 576×8, zero ink. `paint_seq` on an empty slice still
  calls `first_baseline(…, GRID)`.
- **Principle:** Engine finding (P1 §5, P3 §3–4). Tschichold —
  empty slug that looks like a mistake. On a printer this is 1 mm
  of paper.
- **Suspect:** `paint_seq` empty arm; `paint()` `finish(slug_bottom)`.
- **Severity:** rhythm
- **Fix:** If `sheet.frames` and `sheet.notes` are both empty, emit
  height 0 (or refuse). Do not charge GRID for nothing.

### 11. Inline stacked math does not grow the body slug

- **Stems:** `ext-math-a-inline` (height 37 — one Plus2 slug)
- **PNG:** `x+y` and `\frac{a}{b}` share one body line. Numerator
  sits at the top of the tape; denominator rides below the prose
  baseline. Next-line collision is avoided only because the file is
  one line.
- **Principle:** Engine finding (P2 §4). Tschichold — leading in
  proportion; no crashing glyphs.
- **Suspect:** `line_metrics` sees math ascent/depth, but
  `first_baseline` is still clamped to `TextSize::Pt11.skip_dots()`
  (37) for the block.
- **Severity:** rhythm
- **Fix:** Let the line slug be `max(size.skip_dots(),
  ceil(ascent+depth)+GRID)` when a `Piece::Math` is present.
  `paint_run` already has `(ascent, depth)`.

### 12. Display math / long sums do not wrap; `\frac` in notes is a hole

- **Stems:** `ext-math-b-display`, `ext-math-e-zoo`,
  `ext-math-d-in-note`
- **PNG:** Narrow `x` is centered (correct). `1+…+24` is centered
  and ink runs to ~574 — one line, no wrap. Zoo: `\sum` is heavy,
  limits tight on the bars; matrix is indented/centered under a
  left-aligned line. Note math `½` has word spaces around it
  (issue 2) and sits in the 8 pt slug.
- **Principle:** Engine finding (P2 §5). Centering is the dialect
  (keep). Full-measure unwrapped display is a measure problem.
- **Suspect:** `paint_math` — blit at leftover/2, no wrap.
- **Severity:** rhythm (zoo / long sum) / color (sum weight)
- **Fix:** If `math.width > measure`, do not center a clip — either
  reject (`could not typeset math`) or scale down once. Do not wrap
  RaTeX bits. Leave narrow display centered.

### 13. Numeric columns are end-aligned, not decimal-aligned

- **Stems:** `ext-table-h-numeric`, `doc-a-receipt` (price col),
  `ext-table-a-two-col` (aligned only because both prices are `.00`)
- **PNG:** `2.00` / `12.50` / `4.5` / `1` share a right edge.
  Decimal points walk. Tabular figures are used (`Digits::Tabular`
  on `ColAlign::End`).
- **Principle:** Engine finding (P2 §3). Tschichold — if the design
  claims figures, decimals line up. The engine does **not** claim
  `DecimalDelim` for tables (that enum is list markers only).
- **Suspect:** `col_digits` + `Cell::ink_x` — End only.
- **Severity:** align (not a blocker unless we advertise decimals)
- **Fix:** Optional `ColAlign::Decimal` that splits on `.` and hangs
  the fractional part. Do not invent it in the corpus; add it only
  if house style wants money columns to lock.

### 14. `ftp://` autolinks; bare `www` allocates a note

- **Stems:** `ext-never-b-autolink-off-cases`, `cm-6.5-b-gfm-autolink`,
  `doc-c-changelog`
- **PNG:** `www` stays roman. `ftp://files.example.com` is italic,
  dest==text, wraps, no note. Bare `www.example.com` is italic plus
  note `1. http://www.example.com` because dest ≠ text.
- **Principle:** Engine finding (P1 §11, P2 §6, P3 §7). Dialect —
  crib says autolinks are italic with no note.
- **Suspect:** comrak `extension.autolink` + `lower.rs` `note_for_dest`.
- **Severity:** dialect
- **Fix:** Engine only: treat GFM `www` dest as equal to text (no
  note), and do not italicize `ftp://` if the crib is law. Do not
  rewrite the corpus; it is the witness.

### 15. Escaped `\[x\]` is display math

- **Stems:** `cm-6.3-d-brackets`
- **PNG:** `[x]` flush left (literal). Centered italic `x` (display
  math from `\[x\]`). `[lookalike]` flush left. Large vertical slugs
  around the centered x — display math’s GRID air.
- **Principle:** Engine finding (P1 §10). Dialect. Centering is
  correct for display math; the surprise is the source mapping.
- **Suspect:** `math_latex` / `paint_math`.
- **Severity:** dialect
- **Fix:** Document in the skill. If literal brackets are required,
  the corpus must avoid `\[…\]`. No engine change unless we add a
  different display-math fence.

### 16. Note-rule / body-rule rhythm: rules sit in the previous leftover

- **Stems:** `cm-4.1-a-break-markers`, `set-edge-c-ends-rule`,
  `doc-d-spec-sheet`, all `set-pair-*` (terminal rule)
- **PNG:** Full-measure 2-dot rules (576). Air from the previous
  baseline down to the rule is the leftover of that frame’s slug;
  air after the rule is a full `next` slug. Letters with ascenders
  (`b`, `d`) sit tight under the rule; the rule sits far under the
  previous baseline. `NOTE_RULE` (144) above notes is the opposite
  problem — short, left, clearly a different voice (keep).
- **Principle:** Tschichold — a rule glued to nothing / uneven
  leading around rules. Vignelli — the rule is structure, so it
  should sit in the *middle* of the inter-block air, or have equal
  GRID above and below.
- **Suspect:** `extra(Prose|Hang, Rule) => 0` then `paint_rule` at
  `slug_bottom`; `Place::Rule` then charges `next` to the following
  frame.
- **Severity:** rhythm
- **Fix:** Give Rule a GRID (or half-slug) *before* paint, and the
  same after. `extra(_, Rule) => GRID` and `extra(Rule, _) => GRID`
  (keep the existing Cols exception if compact tables must kiss).

### 17. Quote hang is GRID-true and optically silent

- **Stems:** `cm-5.1-a-quote-basic`, `cm-5.1-b-quote-lazy`,
  `cm-5.1-c-quote-nested`, `set-nest-a-quote-cap`,
  `ext-foot-e-in-quote`
- **PNG:** Ink starts at x=8 or 9 (one GRID), then 16, then 24 at
  three-deep. No bar. At 1× the hang is invisible; the tape reads
  as flush-left body. Nested quotes *do* staircase when the words
  are short (`one` / `two` / `three`). Long sentences in
  `set-nest-a` wrap to a narrower measure — the hang is doing
  measure work even when the left edge is hard to see.
- **Principle:** Vignelli reduction (no bar) is good. Tschichold
  hanging indent “one voice” is weak at 8 dots.
- **Suspect:** `paint_quote` `x0 + GRID`.
- **Severity:** hierarchy (optical), not a bug
- **Fix:** Do **not** add a bar. Optional: hang 2×GRID (16) for
  depth 1 so the voice is visible on 80 mm paper. Leave GRID as the
  nest increment.

### 18. List mark column is correct; dash gutter looks wide next to `1.`

- **Stems:** `cm-5.3-e-nested-mixed`, `cm-5.3-f-runover`,
  `cm-5.3-b-ordered-start`, `set-nest-b-list-marker-width`,
  `set-notes-a-many`
- **PNG:** Dash and task sit at the start of a hang that is at
  least as wide as `"10."` (`List::mark_width`). Content of dash
  items therefore starts farther right than `1. ol`. Runover of a
  dash item and of `100.` both clear their *own* mark; they do not
  share one content edge across list kinds. Two-digit notes
  (`set-notes-a`) right-align in the mark band — `1.` starts ~x=15,
  `12.` starts ~x=2; URL column holds at ~42. `999.` at three deep
  hangs correctly once the corpus uses a blank line (P3 §8).
- **Principle:** Tschichold — consistent indent, wrap clears the
  mark. The shared hang (dash width = two-digit width) is
  intentional and good. The *look* of a wide hole after `–` is the
  cost of that unification.
- **Suspect:** `List::hang_dots` / `paint_list`.
- **Severity:** align (accept) — do not shrink dash hang below
  `10.` or mixed lists break.
- **Fix:** None. Document the shared mark column. The `x`
  paragraphs in `cm-5.3-b` are authoring, not an engine bug.

### 19. Head after head / mark after mark: large leftover slugs

- **Stems:** `cm-4.2-a-atx-levels`, `cm-4.2-b-atx-forms`,
  `set-pair-c-after-mark`
- **PNG:** One 18 pt masthead, five identical 11 pt bold heads —
  no ladder (correct). `extra(Head, Head) => next` puts a full body
  slug between stacked heads, so `Two`…`Six` look double-spaced.
  Empty ATX in `cm-4.2-b` is a large white hole, then a masthead
  `many spaces`. Mark→next uses `mark_slug` (51), so pair tapes
  show a big air after `Mark` before `– list`.
- **Principle:** Vignelli — no accidental in-between size (good).
  Stacked heads of the same rank should sit closer than head→prose
  of a new movement. Empty heading as a slug is an orphan.
- **Suspect:** `extra(Head, Head|Mark) => next`; empty `paint_mark`
  / `paint_head` still take a slug.
- **Severity:** rhythm / hierarchy
- **Fix:** `extra(Head, Head) => 0` (or GRID). Drop inkless empty
  headings to zero height.

### 20. Figure extreme aspect and measure-edge bars

- **Stems:** `set-fig-b-measure-edge`, `set-fig-c-extreme-aspect`,
  `set-fig-d-dither`, `set-fig-a-native`, `cm-6.4-a-images`
- **PNG:** `w575` / `w576` / `w577` are three solid black bands;
  first is 575 wide (1-dot right paper). `vline` is a 1200-dot
  centered hairline (tape 1225); `hline` is a 1-dot full-measure
  rule. Native `sq60` is a centered hollow square (4 px border) —
  correct, not tofu. Dither: left of `ramp` and top of `gray` go
  solid black before the Floyd–Steinberg field (ordered luma 0).
- **Principle:** Centering figures is dialect (keep). A 1200-dot
  1-px rule is an orphan of structure on paper — accepted as the
  stress test. Solid-black lead-in on ramps is the dither of luma 0,
  not the alpha bug.
- **Suspect:** `paint_figure` / `Figure::from_image`.
- **Severity:** color/density (ramp lead-in) — accept; extreme
  aspect is the test.
- **Fix:** None for vline/hline. If ramp lead-in bothers, dither
  from luma 1…254 only in the asset generator — that is corpus
  asset, not engine, unless `from_image` special-cases 0.

### 21. `~~strike~~` prints tildes; smart quotes curl

- **Stems:** `ext-never-a-strike`, `ext-smart-a-quotes`,
  `ext-smart-b-dashes`
- **PNG:** `~~X~~` literal (extension off — correct). Quotes in
  `ext-smart-a` are curled (the 1× is easy to misread as straight;
  apostrophes in `It’s` / `Bob’s` are typographic). En / em / `1–10`
  are correct; `...` is three periods, not U+2026.
- **Principle:** Dialect (keep strike off). Tschichold — hanging
  quotes would hang the opening `“` into the margin; we do not, and
  should not on a 0-margin tape.
- **Suspect:** smart punct in the lower.
- **Severity:** dialect (ellipsis is the only soft miss)
- **Fix:** Optional: map `...` → `…` in the same smart pass as
  dashes. Do not hang quotes.

### 22. Reject stems that parse are not engine paint bugs

- **Stems:** `rej-table-c-ragged` (p4 preview: header `a b c`, body
  `1 2` — comrak padded), `rej-foot-a-undefined` / `rej-foot-other`
  (literal `x[^n]`), `rej-tableXXXX` (raw pipes as prose)
- **PNG:** These printed because they are not in `reject/`. P4
  findings already dropped the unreachable arms.
- **Principle:** Engine finding (P4).
- **Severity:** dialect (harness)
- **Fix:** None in compose. Keep them out of `reject/`.

---

## 4. What is already good

Do not “fix” these. They are the Vignelli reductions the tape is for.

- **Two faces, three sizes.** Masthead 18 / body+heads 11 / notes 8.
  `cm-4.2-a` is the specimen: one display line, five identical bold
  heads. Flattened heading interiors (`cm-4.2-c`) are correct.
- **No blockquote bar.** Quote is a hang, not a decoration.
- **No table rules, no cell borders.** `ext-table-a-two-col` and
  `doc-a-receipt` — label left, money right, 8-dot gutters. Compact
  vs hang in `cols.rs` is real structure.
- **`NOTE_RULE` = 144 = 18×GRID.** Short, left, on-grid. Distinguishes
  the apparatus from a section rule. Full-measure note rules would
  shout.
- **Notes are 8 pt, numbered, hanging.** Title then URL; wrap of the
  title clears the mark (`set-notes-c`). Two-digit marks right-align
  in the band (`set-notes-a`). `mailto:` stripped (`cm-6.3-c`).
- **Duplicate URLs share a number; dest==text has no note**
  (`cm-6.3-c`). Keep. The `www` exception is dest≠text, not a paint
  bug.
- **Task boxes on the grid** (`ext-task-a-basic`). Nested loose
  tasks use real slugs (`ext-task-b-nested-loose`).
- **Tight vs loose lists** (`cm-5.3-c`) — the only optional air.
  Head sits on following prose (`extra(Head, Prose)=0`) — Tschichold
  “the head belongs to what follows.”
- **Code never wraps; it clips.** Hung 8–11 dots (GRID + Menlo
  sidebearing). Info strings paint nothing (`cm-4.5-b`). That is
  the dialect — authors reflow to ~30 columns.
- **Figures center, never upscale.** `sq60` hollow square is the
  asset, not tofu. Floyd–Steinberg on `photo.jpg` / `ramp` is even
  enough for 1-bit.
- **NFC = NFD; `fi`/`fl` liga; Greek and Cyrillic paint.** CJK /
  Arabic / emoji tofu is honest (`cm-6.9-b`, `cm-6.9-c`).
- **`$4.50` is money; `\(x\)` is math** (`ext-math-f-dollars`).
- **Pair matrix** (`set-pair-a`…`j`) is the same ten frames in
  rotation. Heights 603–691 track `extra()` — Mark air, Rule air —
  not random slugs. `set-tall-a` (3077) keeps full-measure rules
  and a centered figure without falling apart.

---

## 5. Priority for an implementation pass

Engine edits only. Do not change corpus files to hide a defect.

1. **`paint_line` inter-piece space** (issue 2) — one flag on
   `Piece`. Fixes the model receipt and every noted / autolinked
   stop. Highest human notice per line of code.
2. **Clip table overflow to the cell** (issue 1) — `paint_line` or
   blit scissor. Unreadable tape today.
3. **Wrap or hyphenate overlong prose/note tokens** (issue 3) —
   `wrap_chunk_plan` only; leave `paint_code` clipping.
4. **Alpha composite onto paper** (issue 4) — `Figure::from_image`.
5. **TAB → columns in `paint_code`** (issue 5).
6. **NUL → visible FFFD or reject** (issue 6).
7. **Quote-scoped rules** (issue 7) — full tape or named short.
8. **Hang-frame seam** (issue 8) — sibling list/quote/code get one
   slug; intra-list stays 0.
9. **Empty item / empty sheet slugs** (issues 9–10).
10. **Inline math line slug** (issue 11); display-too-wide reject
    or scale (issue 12).
11. **Rule air** (issue 16) — GRID above and below.
12. **Stacked-head slug** (issue 19).
13. **Dialect only, if the crib wins:** `ftp://` literal; `www`
    dest==text (issue 14). No corpus rewrite.
14. **Do not do:** decimal aligner (13) unless house style asks;
    quote bar (17); shrink dash hang (18); hanging quotes (21);
    ellipsis mapping unless it rides a smart-punct commit.

---

## Inspected vs found

| Set | Found | Opened |
| --- | ---: | ---: |
| Goldens | 123 | 123 |
| p1 / p2 / p3 previews (same stems, 2×) | 121 | sampled 2× + all matching goldens |
| Extra stems (p4 + `target/corpus-preview`) | 12 | 10 (`04-code-fence`, `mono-ruler-left` not opened) |
| **Unique stems** | **135** | **133** |

**Could not open:** none (every golden decoded). Not opened as
separate files: `target/corpus-preview/04-code-fence.png`,
`target/corpus-preview/mono-ruler-left.png`.
