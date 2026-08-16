# tm20

ESC/POS for Epson TM-T20 printers, proven on the TM-T20III (`04b8:0e28`).
Two crates, one way of depending. There is no printer builder and no CUPS.

- **`tm20`** — protocol. A `Document` of `Command` values encodes to bytes; a
  `Transport` writes them. USB, serial, TCP. `Command::Text` is CP437.
  The printer never sees a font.
- **`tm20-set`** — typesetter. A `Sheet` of `Frame`s (flush left, Vignelli
  sizes, leading on an 8-dot grid) compiles to one `tm20::Graphics`. OpenType
  lives here. Receipts with Grotesk are a `Sheet` or they do not exist.

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
cargo run -p tm20-set -- print ticket
cargo run -p tm20-set -- print nhg
```

`publish = false`. License is 0BSD.
