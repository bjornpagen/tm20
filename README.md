# tm20

ESC/POS dialect for Epson TM-T20 printers. Proven on the TM-T20III
(`04b8:0e28`). A `Document` of `Command` values encodes to bytes; a
`Transport` writes them. There is no printer builder and no CUPS.

This is one encoding per job. Commands the T20 firmware ignores or that
have a newer replacement stay unrepresentable. Other 80 mm Epson TMs that
speak the same modern subset (`GS ( L`, function-B barcodes) should work;
USB open is hardcoded to the T20III product id until someone adds another.

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

Host OpenType faces load from bytes, a path, or the system font database
and raster to `Graphics`. The printer never sees a TTF.

```
cargo run -- hello
cargo run -- test all          # skip --wait if bulk IN is dirty
cargo run -- test typeface
```

`publish = false`. License is 0BSD.
