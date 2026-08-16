# tm20-set tests

Paper cases for the typesetter, the same job as `tm20`’s encode goldens and
selftest catalog. `cargo test -p tm20-set` does not need a printer. It loads
a [`FaceTable`] from the system sans, the same way the binary’s ticket sheet
does. It does not name Neue Haas Grotesk.

## What is tested

- **`algebra.rs`** — compose adjacency: paragraph blank line, Head sticks, Mark
  air, rule slug, pair hang from a rule, list mark column, tape width, pair ink
  on both sides. Missing cuts error.
- **`lower.rs`** — a Sheet lowers to Init, PC437, Graphics, Feed 3, partial
  cut. No `PrintSpeed`. Encoded bytes contain `GS ( L`.
- **`frames.rs`** — every `Frame` variant composes and encodes (the catalog
  analog). A new variant fails the exhaustive match until a case is added.

Constructor checks that do not need a face stay next to the type:
Plus2 = 37 dots at 11 pt.

## Mechanism vs policy

The library takes a `Sheet` of `Cut`s and a `FaceTable`. It does not read
files or look up a family. Tests and the `tm20-set` binary load bytes (system
sans or `~/Library/Fonts`) and insert them into the table.

## Residuals the engine still chooses

These are this typesetter’s rhythm, not house-face policy. Tests assume them:

- text leading is Plus2 (11 pt → 37 dots)
- `HANG` is 3 dots (type under a form rule)
- `GRID` is 8 dots
- Head has space above, none below, and always uses Bold upright
- shaping always asks for `kern liga calt`
