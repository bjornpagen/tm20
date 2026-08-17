# tm20-set tests

`cargo test -p tm20-set` does not need a printer. It loads a [`FaceTable`] from
the system sans plus **Commit Mono** (`~/Library/Fonts/CommitMono-400-Regular.otf`).
It does not name Neue Haas Grotesk.

Constructor checks that do not need a face live next to the type. Compose
proofs live in `tests/`. Each test is one fact.

## Next to the type

- **`size`** — 11 pt = 31 dots, Plus2 skip 37; 8 pt Plus2 29; display 18 solid 51
- **`leading`** — `GridSkip` on 8; Plus1 at 11 pt is 34; Solid equals body; `TASK_BOX` 24; `NOTE_RULE` 144
- **`frame`** — `Measure::new(0)` is none; ragged figure bits and bad image bytes error; PNG scales to the measure
- **`face`** — garbage bytes are `Error::Font`

## Compose (`algebra.rs`)

Tape width. Closed sizes. Wrap taller than one line; first line hugs the measure. Paragraph blank vs hard break. Head sticks; Mark has more air, can center, tracking widens. Rule clears the slug; Two is two rows. Columns: ink both sides, hang from a rule (text does not), consecutive tables tight, start column wraps, three columns compose, illegal shape errors. List: dash runover clears the mark column; decimal hang fits the widest marker; loose taller than tight; tight item texts share a slug; nest and ul-then-ol are not a blank; nest cap 3; task box hangs on the grid and sits in the cap band; a check adds ink. Quote hang is `GRID`; nest cap 3. Code hang is `GRID`, does not wrap, and needs Mono. Figure blits. Notes after a short rule with 2 pt air. Missing text/display/mono cuts error. Preview PNG is a PNG.

## Catalog analog (`frames.rs`)

Every `Frame` variant lowers and encodes. Mixed cuts live on the `Text` case. A new variant fails the exhaustive match.

## Lower (`lower.rs`)

Init, PC437, Graphics, Feed 3, partial cut. No `PrintSpeed`. Bytes start with `ESC @` and contain `GS ( L`.

## Residuals the engine still chooses

- text leading is Plus2 (11 pt → 37 dots)
- `HANG` is 3 dots (type under a form rule)
- `GRID` is 8 dots
- Head has space above, none below, and always uses Bold upright
- shaping always asks for `kern liga calt`
