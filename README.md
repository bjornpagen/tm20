# tm20

ESC/POS for Epson TM-T20 printers, proven on the TM-T20III (`04b8:0e28`).
Protocol, typesetter, markdown. There is no printer builder and no CUPS.

Pinned to **nightly-2026-08-15** (`rust-toolchain.toml`), same dated nightly
as bumbledb. Edition 2024. `cargo clippy --workspace --all-targets -- -D warnings`
is the lint gate (`all` + `pedantic`, `unsafe_code = deny`).

Crates.io, 0BSD, **0.1.0**:

```toml
tm20 = "0.1"
tm20-set = "0.1"
tm20-md = "0.1"
```

The `tm20-set` binary lives in unpublished `tm20-cli` so markdown can depend
on the typesetter without a crate cycle.

- **`tm20`** — protocol. A `Document` of `Command` values encodes to bytes; a
  `Transport` writes them. USB, serial, TCP. `Command::Text` is CP437.
  The printer never sees a font. `cargo install tm20` is the protocol CLI.
- **`tm20-set`** — typesetter. A `Sheet` of `Frame`s (flush left, Vignelli
  sizes, leading on an 8-dot grid) compiles to one `tm20::Graphics`. OpenType
  lives here as bytes. Which face those bytes came from is the program that
  prints, not the library.
- **`tm20-md`** — CommonMark 0.31.2 plus pipe tables, task lists, autolinks,
  footnotes, and LaTeX math (`\(inline\)`, `\[display\]`), walked into a
  `Sheet`. Dollars stay currency. comrak and RaTeX live here. This crate does
  not depend on `tm20`.

`tm20-set` depends on `tm20`. Never the reverse.

```rust
use tm20::{encode, Command, CutKind, Document, Transport, Usb};

let doc = Document::new(vec![
    Command::Init,
    Command::Text("SYSTEM ONLINE".into()),
    Command::Feed { lines: 3 },
    Command::Cut { kind: CutKind::Partial },
]);
Usb::open(None)?.write(&encode(&doc)?)?;
```

This is one encoding per job. Commands the T20 firmware ignores or that have
a newer replacement stay unrepresentable. Other 80 mm Epson TMs that speak
the same modern subset (`GS ( L`, function-B barcodes) should work; USB open
is hardcoded to the T20III product id until someone adds another.

```
cargo run -p tm20 -- hello
cargo run -p tm20 -- test all          # skip --wait if bulk IN is dirty
cargo run --bin tm20-set -- print ticket
cargo run --bin tm20-set -- print nhg     # needs Neue Haas Grotesk in ~/Library/Fonts
cargo run --bin tm20-set -- print md path.md
cargo run --bin tm20-set -- print md crates/tm20-md/fixtures
```
