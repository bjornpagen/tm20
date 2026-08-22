# Representation pass — the review's defects made unexpressible

Follow-up to `02-typography-review.md`. Doctrine: change the data before the
control flow (Brooks ch. 9; Pike rule 5; Minsky's illegal states; King's
parse-don't-validate). Each fix below names the representation that absorbed
it. Gate green: clippy `-D warnings`, full test suite, snap compare against
re-blessed goldens.

## Type-level

- **`Cut` split.** `Cut` was seven variants covering both optical roles, so a
  Light paragraph and a Mono masthead were representable and had to be
  guarded. Now `Cut` is the five text voices and `DisplayCut` is
  `Roman | Light`; `Mark.cut: DisplayCut`. The dead `Medium` variant is gone.
- **`Kit` at the compose boundary** (parse, don't validate). `FaceTable`
  lookups returned `Result` at every shape site — validation re-run on every
  use, the proof discarded. `FaceTable::kit()` parses once; `Kit.text(cut)`
  is total. The `Result`s fell out of the whole wrap/measure/cols family
  (`cols::layout` cost is now `-> f64`), and `Error::MissingText` fires once,
  at entry. The cli kit's trailing per-cut checks were deleted — they were
  the validator that threw its proof away.
- **`Cursor::baseline` is one method.** `first_baseline`/`later_baseline`
  differed only in an `unreachable!` guard; `Place` already carries the
  frame-boundary state (the sentinel). Every `if li == 0` branch and
  conditional `flush_marks` at six paint sites is gone.
- **`Code::new` detabs at construction.** Tabs were expanded in the markdown
  walk *and* re-expanded in paint. Construction is now the parse; paint
  trusts.

## Rhythm as data

- **`pair()` speaks for rules and heads.** `(_, Rule)` and `(Head, Head)`
  are `Seam` — one module of air; `(Mark | Cols, Rule)` stays `Stick` (a rule
  kisses the masthead it opens and the table it totals). After a rule,
  everything but a hanging table takes one module. Review issues 16 and 19.
- **`bump` raises the whole slug.** Air moved only the pen; a rule reads
  `slug_bottom`, so `(X, Rule)` pairs changed nothing. Both fields move now.
- **`QUOTE_HANG: [16, 8, 8]`.** The first quote voice is two modules so it
  reads on 80 mm paper; each nest adds one. A table, not a branch on depth.
  Review issue 17 (the optional fix; the bar stays banned).

## Line as data

- **`Glue::Break`.** A token wider than the measure explodes at URI
  punctuation (`/ ? & = # - _ . , ; : @`) into fragments joined by a
  zero-width `Break`; `Words { items, joins }` carries the boundary glue and
  the wrap DP charges a space only across `Space` joins. A fitting token
  never shapes in pieces, so kerning inside it is untouched; a token with no
  punctuation still clips, honestly. Prose and note URLs now wrap. Review
  issue 3 (fenced code still clips — that is the dialect).
- **Roman word space** (inherited from the adjacency commit, kept): a
  `Glue::Space` is the Roman space at that size, whatever cut spoke last.

## Boundaries

- **U+0000 → U+FFFD before the parse** (`tm20-md::sheet`). comrak dropped the
  NUL and left an invisible seam between two words; the boundary now makes it
  a visible replacement. Review issue 6.
- **Bare autolinks never note.** A GFM `www.…` autolink's dest is its own
  text plus a scheme comrak added; `note_for_dest` strips `http(s)://` before
  the text comparison, so the crib's "autolinks carry no note" is true again.
  Review issue 14 (`ftp://` stays italic; the fact is dest == text).
- **Nothing charges nothing.** An empty sequence paints no ink and takes no
  slug: the empty sheet is one blank row, not a GRID of paper. An empty note
  definition still owns its number and slug so its mark cannot leak onto the
  next note. Review issue 10.

## Not done, on purpose

Decimal-aligned money columns (review 13 — only if house style asks), the
quote bar (17), shrinking the dash hang (18), hanging quotes (21), and any
comparison threshold in the snap harness. Inline `` `code` `` spans break
like prose tokens when overlong; if the dialect wants them atomic, that is a
one-line `Piece` fact to add with its corpus witness.

## Incident note

While this pass was being applied, the previous fix-wave agent was still
alive and re-asserting its uncommitted buffers over freshly edited files; it
finally committed everything as 98501da and went quiet. This pass was
re-applied on top of that commit. If two agents must share this tree again,
serialize them.
