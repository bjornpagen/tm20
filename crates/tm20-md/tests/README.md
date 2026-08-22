# tm20-md snap suite

Pixel-exact visual regression for markdown → `Sheet` → 576-dot raster.
The PNG *is* the golden: decode it and you have the bits. Comparison is
exact equality. The paint pipeline is deterministic (unhinted fontdue at
integer ppem, harfrust shaping, fixed Floyd–Steinberg), so a pixel
difference is a real change.

`paper.rs` still owns `fixtures/`. This corpus is additive.

## Layout

    tests/
      snap.rs               harness
      common/mod.rs         house FaceTable + faces.lock digest
      faces.lock            FNV-1a 64 of Helvetica.ttc and Menlo.ttc
      corpus/*.md           one concern per file
      corpus/assets/*       images the corpus names
      goldens/<stem>.png    1× grayscale, 576 px wide
      reject/*.md           inputs that must fail
      reject/expect.txt     stem = exact Error Display string

## Commands

Compare (the default):

    cargo test -p tm20-md --test snap

Bless missing or changed goldens and rewrite `faces.lock`:

    TM20_SNAP=bless cargo test -p tm20-md --test snap

Anything other than `bless` is compare mode. A second bless with a
clean tree writes nothing.

On mismatch the harness writes a Playwright-style triplet to
`target/snap/`:

- `<stem>.expected.png`
- `<stem>.actual.png`
- `<stem>.diff.png` — paper white, matching ink #666666, added #DD0000,
  removed #0066CC

Regenerate corpus assets (ignored test; binaries are committed):

    cargo test -p tm20-md --test snap -- --ignored write_corpus_assets

Preview one file without a printer:

    cargo run --bin tm20-set -- --dry --png target/corpus-preview print md path.md

## faces.lock

Two lines, `helvetica <hex>` and `menlo <hex>` — FNV-1a 64 over the font
file bytes. Every snap test checks the lock first and fails with
`font drift — inspect and re-bless` when a macOS update moves the faces.
That is a conscious re-bless, never a mystery diff.

## Adding a corpus file

1. Author `tests/corpus/<area>-<letter>-<slug>.md`. One fact. No raw HTML
   (including comments). Intent lives in the filename.
2. Preview with the CLI command above. If the tape looks wrong, that is
   an engine bug — do not rewrite the markdown to hide it.
3. `TM20_SNAP=bless cargo test -p tm20-md --test snap`
4. Read the new `tests/goldens/<stem>.png`. Commit the markdown, the
   golden, and any new asset together.

A reject file is the same motion with `tests/reject/` plus one
`stem = exact Display` line in `expect.txt`. Verify the string by
running before you commit. Two sketched rejects are omitted on purpose:
a ragged table row is padded by comrak, and an undefined footnote
stays literal — both parse, so they cannot live in `reject/`.

## Tests in snap.rs

- `corpus_matches_goldens` — compose every corpus file; exact raster match
- `rejects_reject` — every reject file’s `Display` equals `expect.txt`
- `faces_are_locked` — lock check with a readable message
- `compose_is_deterministic` — one seed, twice, identical bits

Band splitting is not snapshotted. `lower()` is a pure row partition;
`paper.rs` pins that property. `set-tall-a-band-cross` keeps the >910-dot
path exercised end to end.
