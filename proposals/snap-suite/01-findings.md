# Snap-suite findings

First-bless observations for human adjudication. Not adjudicated.
Categories in brackets: observe / suspect golden / spec-code discrepancy /
corrected expect / skipped / authoring.

1. [spec-code discrepancy] After the §2 purge of the eight named files,
   `rg -iE "commit ?mono"` still hits `proposals/snap-suite/00-proposal.md`
   (the purge section itself). The eight touch points are clean. §8
   acceptance says "No `commit ?mono` match anywhere in the repository,"
   which cannot hold while this proposal remains.

2. [observe] `crates/tm20-md/fixtures/04-code.md` verification: Menlo paints
   (dotted zero, slab `1`). The fenced hang line
   `fn hang(x0: u16) -> u16 { x0 + 8 }` clips — the preview ends at
   `{ x0 +` and ` 8 }` is gone. The sentence labeled "This line is meant to
   overflow the tape" is body Helvetica, not a code block, and wraps instead
   of clipping.

3. [observe] `alpha.png` (§3.7 observe): an opaque black circle (r=24,
   center 32,32) on fully transparent ground. `Figure::from_image` lumafies
   via `image::to_luma8`, which drops alpha and keeps RGB. Transparent
   pixels are written as `(0,0,0,0)` and the circle as `(0,0,0,255)`, so
   both become luma 0. A dry compose of the asset is a solid 64×64 black
   square, centered; the circle is not visible.

4. [observe] §4.1 `cm-2.3-a-insecure`: U+0000 does not paint a visible
   U+FFFD tofu. The raster is `before` abutting `after` with only a ~2-dot
   gap in the same range as other letter sidebearings, no replacement box.
   Either comrak strips NUL or Helvetica draws FFFD with no ink. Contrast
   `cm-6.9-c-tofu`, where an explicit U+FFFD is a hollow square.

5. [spec-code discrepancy] §4.1 `cm-2.2-a-tabs-code`: a tab inside a fence
   survives as U+0009 in the code literal, but Menlo has no tab glyph.
   `paint_code` / `TextFace::shape` emit a .notdef tofu (`col▯umn`), not a
   column advance. The proposal’s “survive as columns” is not what compose
   does.

6. [observe] §4.1 `cm-2.5-a-entities`: `&nbsp;` is a visible ~11-dot gap
   between `a` and `b`, wider than a normal word space. `&foo;` stays
   literal. Named and numeric entities paint as `& < # © ©`.

7. [authoring] §4.1 `cm-4.7-b-ref-edges` cannot share a file with used
   refs. Split into `cm-4.7-b1-dup-unused` (first def wins; unused def
   invisible) and `cm-4.7-b2-only-defs`. Corpus is 52 new `cm-*.md` files,
   not the proposal’s single `cm-4.7-b` stem.

8. [observe] §4.1 `cm-4.7-b2-only-defs`: a document of only reference
   definitions composes an 8-dot empty tape (GRID slug, no ink). Unused
   defs are dropped; no notes apparatus.

9. [observe] §4.2 `cm-5.2-c-item-empty`: an empty item between two full
   ones paints an en-dash with no text — a short mark-only slug between
   the labeled rows. `paint_list`.

10. [observe] §4.2 `cm-5.2-d-item-heading`: `##` inside a list item paints
    as an 11 pt bold Head after the dash, not an 18 pt masthead. Same size
    as body heads. `paint_head`.

11. [observe] §4.2 `cm-5.3-a-markers`: `-` `*` `+` all print en-dashes.
    Marker switch starts new lists in the AST, but `extra(Hang, Hang)` is 0
    so there is no visible seam — three lists look like one tight list.
    `compose::extra`.

12. [authoring] §4.2 `cm-5.3-b-ordered-start`: blank lines do not split
    same-delimiter ordered lists (one loose list; first start wins).
    Inserted `x` paragraphs so starts 1, 0, 7, 999999999 and `1)` stay
    separate lists. The 9-digit marker takes a much wider hang; content
    still clears it.

13. [spec-code discrepancy] §4.3 `cm-6.3-d-brackets`: the proposal’s
    escaped `\[x\]` is display math (`math_latex`), not literal brackets.
    It paints a centered italic `x`. `paint_math`. Shortcut-lookalike
    `[lookalike]` and bare `[x]` stay literal.

14. [spec-code discrepancy] §4.3 `cm-6.5-b-gfm-autolink`: the dialect crib
    says autolinks are italic with no note. Bare `www.example.com` has dest
    `http://www.example.com` ≠ text, so `note_for_dest` allocates note 1.
    Bare `https://…` and `user@example.com` stay note-less (dest/stored
    equals text). Trailing `.,;:)` are excluded from the link but sit after
    an ~11-dot word-space (see 38).

15. [observe] §4.3 `cm-6.9-a-unicode`: NFC and NFD `café` read as the same
    word at the same bbox. A pixel compare of the two `é` rasters is not
    bit-identical — NFD’s combining acute is a taller spike. `fi` / `fl`
    sit tight (`liga`). `¶ • …` all paint; the bullet reads as a centered
    dot at this size.

16. [observe] §4.3 `cm-6.9-b-scripts`: Greek `Ελληνικά` and Cyrillic
    `Русский` paint. CJK `漢字` is two .notdef tofu boxes; Arabic
    `العربية` is seven tofu boxes. Glyphs are not omitted. No visible RTL
    reordering of the tofu run.

17. [observe] §4.3 `cm-6.9-c-tofu`: U+FFFD, unassigned U+0378, and emoji
    U+1F600 all paint the same hollow-square .notdef tofu.

18. [observe] §4.3 `cm-6.2-c-mixed-delims`: `***strong em***` split order
    is not visually distinguishable — em-then-strong and strong-then-em
    are both `Cut::BoldItalic`.

19. [observe / suspect golden] `ext-table-d-overflow` (§4.4): minimum
    widths exceed the 576-dot measure. `cols.rs::overflow` shrinks the flex
    boxes; it does not stack columns vertically (the proposal’s
    “overflow/stacked path” is not a second layout mode). Both unbreakable
    tokens stay on one row per cell and paint past their boxes, so glyphs
    collide across the gutter and reach both tape edges. Two slugs tall
    (header + body), no wrap. After the collision mass, trailing letters
    of the right cell (`opqrs`) reappear at the right edge. `paint_cols` /
    `paint_line`.

20. [observe] `ext-table-g-degenerate` (§4.4): an empty first cell leaves a
    blank under header `a` with `x` in column two; an empty second cell
    leaves `y` under `a` and a blank under `b`. A header-only table prints
    both header cells on one bold line (`only` `header`) and nothing else.

21. [observe / spec-code discrepancy] `ext-table-h-numeric` (§4.4): the
    price column hangs on the tape edge (`ColAlign::End`). Digits are
    tabular, but there is no decimal-point aligner — `2.00`, `12.50`,
    `4.5`, and `1` share a right edge, so the points do not line up when
    the fractional width differs. The proposal cites `DecimalDelim` here;
    that enum is only the ordered-list marker (`1.` / `1)`) in `lower.rs`,
    never a table column policy.

22. [observe] `ext-math-a-inline` (§4.4): short \(x+y\) and a stacked
    `\frac{a}{b}` share one body line. Leading does not grow past the
    normal 37-dot Plus2 slug. The fraction fills that slug (numerator near
    the top of the tape, denominator below the prose baseline).
    `paint_math`.

23. [observe] `ext-math-b-display` (§4.4): narrow `\[x\]` is centered on
    the measure (`paint_math` halves leftover). The long sum `1+…+24` is
    also centered and spans nearly the full 576 dots (ink to ~574); it
    does not wrap.

24. [observe / spec-code discrepancy] `ext-never-b-autolink-off-cases`
    (§4.4): bare `www` stays roman, not a link. `ftp://files.example.com`
    is italic with no endnote (dest == text) and wraps onto the next line
    — comrak’s `extension.autolink` treated it as a GFM autolink. The
    proposal’s fact is that `ftp://` “must stay literal”; the code
    autolinks it. `paint_run`.

25. [observe] `set-wrap-a-long-url` (§4.5): a 100-char bare URL is an
    unbreakable italic autolink token. Wrap parks it on its own line; the
    glyphs clip at the tape edge (`right_gap` 0). "See" stays on the line
    above; "for the source." follows on the line below. No overflow past
    the canvas, no intra-token split. `wrap_chunk_plan` + `blit`.

26. [observe] `set-wrap-b-long-word` (§4.5): the 200-char word clips at the
    tape edge at 11 pt (body) and again at 8 pt (footnote). Neither size
    splits or wraps inside the word. Each run is pushed to its own line
    after the short prefix ("Body:" / "Notes:"). Same `wrap_chunk_plan` +
    `blit` path as (25).

27. [observe] `set-edge-a-empty` (§4.5): a 0-byte file is not an error and
    does not panic. `sheet()` yields no frames; `paint_seq` on an empty
    slice still calls `first_baseline(0, 0, GRID)`, so the tape is 8 dots
    of paper (one grid slug), no ink. CLI: 1 band, `H=8`.

28. [observe] `set-edge-b-blank-only` (§4.5): blank lines produce the same
    empty sheet as (27) — 8-dot blank tape, no ink, no error.

29. [observe] `set-notes-b-long-url` (§4.5): the 120-char destination
    prints as one 8 pt note line and clips at the tape edge (`right_gap`
    0). It does not wrap. Same unbreakable-token clip as (25), at note
    size. `paint_notes` / `blit`.

30. [observe] `set-fig-e-modes` (§4.5): `alpha.png` renders as a solid
    64×64 black square, centered (same luma drop as 3). `indexed.png` is
    an 8-px checkerboard, also 64×64 centered. `paint_figure`.

31. [spec-code discrepancy] `doc-c-changelog` (§4.6 / §1 dialect table): a
    bare GFM `www.example.com/…` autolink is italic *and* emits a note
    (`http://www.example.com/…`). The crib says autolinks are italic with
    no note. `https://example.com/0.4` and `<https://example.com/0.2>` stay
    italic without notes (`dest == text`). Same `www` prefix fact as (14).

32. [authoring] `set-nest-b-list-marker-width` (§4.5): a tight indented
    child starting `999.` does not nest — CommonMark treats it as a lazy
    continuation of the parent item (`999.` does not interrupt; same fact
    as `cm-5.3-d`). The corpus uses a blank line before each child so the
    three `999.` markers actually nest (loose list). Without that, the
    file is one item and the depth/hang fact is invisible. Golden shows
    three nested `999.` levels; markers right-edge in the hang, content
    at ≈ x=73 / 145 / 216.

33. [skipped] `rej-table-c-ragged`. The sketch (3-col header, 2-cell row)
    parses cleanly. GFM/comrak pads a short body row to the delimiter
    width, so `cells.len() == n` and `Error::Cols` never fires. Extra
    cells are dropped the same way. The row-mismatch arm in `lower.rs` is
    unreachable on this parser. A file that parses is a harness failure,
    so the stem is omitted from `reject/` and `expect.txt`.

34. [skipped] `rej-foot-a-undefined`. `x[^n]` with no definition parses
    cleanly: comrak does not emit `FootnoteReference` without a matching
    definition, so the walk never counts a slot and `Error::Note` does
    not fire. This is already pinned by
    `spec.rs::undefined_footnote_stays_literal` and
    `audit/extra-footnotes.md`. Empty `[^n]:` and a different-name
    definition also succeed. Stem omitted from `reject/` and `expect.txt`.

35. [observe / spec-code discrepancy] `rej-image-d-garbage`: dest rewritten
    from the sketch `assets/garbage.png` to
    `../corpus/assets/garbage.png`. Dest resolves relative to `reject/`,
    which has no `assets/`; the sketch path is a missing-file load, not a
    decode. The rewrite hits `Figure::from_image` on `garbage.png` (64
    bytes of `0xA5`). Display is still `could not load figure` —
    `tm20_set::Error::Image` maps to `tm20_md::Error::Image` as §1
    predicted, not `could not decode figure`. No expect-string correction;
    the skill lists a distinct decode string that this path does not use.

36. [suspect golden / spec-code discrepancy] `cm-2.4-a-escapes`: source is
    a backslash before every ASCII punct, including `\[\\\]`. Because
    `math_latex` is on (same fact as 13), `\[…\]` is display math, not
    literal brackets. The golden is two ink bands —
    `!"#$%&'()*+,-./:;<=>?@` then `^_\`{|}~ \A\z` — with a ~128-dot
    inkless hole between them and no `[` `\` `]` glyphs. `paint_math`.

37. [suspect golden] `cm-4.2-b-atx-forms`: `## Closed ##` is 11 pt bold;
    `\# not a heading` is 11 pt roman with a literal `#`. The lone empty
    `#` leaves no glyphs and a ~118-dot inkless hole before the 18 pt
    masthead `many spaces`. Interior five spaces in that masthead collapse
    to one 18 pt word-space. `paint_head` / `paint_run`.

38. [suspect golden] `paint_line` inserts a shaped `" "` between every
    adjacent `Piece` (`i > 0`). Visible wherever a style or note split
    is not a source space:
    - `cm-6.2-b-intraword`: `mid*word*` paints `mid` then a hole then
      italic `word`.
    - `cm-6.2-d-adjacent-runs`: `*a**b**c*` paints three spaced letters.
    - `cm-6.2-e-punct-flanks`: `(*em*)` and `--*em*--` have air around
      italic `em`; `“quoted em ”` has air before the closing quote.
    - `cm-6.5-b-gfm-autolink`: trailing `.,;:)` sit after an ~11-dot
      word-space.
    - `set-wrap-d-note-at-margin`: italic `cited` + superscript `1`, then
      ~16 dots of white, then the sentence period (source has no space
      before `.`). `paint_line`.

39. [suspect golden] `cm-6.3-a-inline-links`: five titled/angled links
    paint italic + superscripts 1–5. `[](https://ex.com/empty-text)`
    leaves no body ink and no superscript, then ~120 dots of blank tape,
    then italic `empty dest` with no note (empty dest). The notes
    apparatus still lists six dests, including `https://ex.com/empty-text`
    as note 6. `paint_run` / `paint_notes`.

40. [suspect golden] `ext-table-f-pipes`: escaped `\|` in the start cell
    paints `a | b`. The code-span cell `` `a|b` `` paints a leading
    backtick plus `a` only; `|b` is not on the tape. Likely the GFM pipe
    split inside the span, then `paint_cols` / `paint_line`.

41. [spec-code discrepancy] `ext-smart-b-dashes`: `--` is an en dash,
    `---` an em dash, `1--10` an en-dash range. Trailing `...` paints as
    three separate periods, not a single ellipsis glyph. The proposal
    lists `...` → ellipsis.

42. [observe] `ext-task-b-nested-loose`: checkboxes and nest indent are
    clear. Vertical air is uneven — the first nested child sits tight
    under `outer done` (~13 dots), then ~44–50 dots before `nested done`
    and `outer open` (loose-list blanks in the source). `paint_list` /
    `extra`.

43. [observe] `set-fig-b-measure-edge`: `w575` is a 575×24 black bar at
    x=0–574 (1-dot right gutter; integer centering of a 575-wide figure).
    `w576` and `w577` are both 576×24 flush. `w577` shrinks to the
    measure at native height; on a solid bar that is identical to `w576`.
    `paint_figure`.

44. [observe] First full-corpus bless (`TM20_SNAP=bless`) wrote 113 new
    goldens and did not rewrite the ten committed seed goldens or
    `faces.lock` (SHA-256 of each seed PNG unchanged). Compare mode is
    green afterward. Goldens remain uncommitted pending adjudication.

Reject `expect.txt` strings that were authored all match the §4.7 table.
No corrected expect strings. Two sketched stems were skipped (33, 34);
authored reject count is 14, not 16.
