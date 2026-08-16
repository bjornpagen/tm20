# tm20 feature board (locked)

This is the decision record for the **protocol crate** (`tm20`: `Document` → `encode` → `Transport`). Higher-level work (Inter, PNG, wrap, gapless art) is a **different crate**. It may depend on `tm20`; `tm20` must not depend on it.

Statuses used below:

- `do` — change this crate in the next implementation pass
- `keep` — already correct; leave it
- `out` — belongs in the host crate, not here
- `later` — real protocol, not in this pass
- `never` — will not add

**One encoding per job.** Obsolete firmware commands stay unrepresentable even if the printer still accepts them.

Hello golden is frozen: `1B 40  1B 74 00  SYSTEM ONLINE  0A 0A 0A  1D 56 42 00`.

---

## Scoreboard

| # | Change | Layer | Status |
| --- | --- | --- | --- |
| 1 | Bitmaps: `GS v 0` → `GS ( L` fn=112+50; rename `Raster` → `Graphics` | protocol | **do** |
| 2 | 1D barcodes: function B only; add CODE93, CODE128, GS1-128 | protocol | **do** |
| 3 | Delete `Reset` (same bytes as `Init`) | protocol | **do** |
| 4 | Delete `Font::C` | protocol | **do** |
| 5 | Rename `Rotate180` → `Rotate90` (`ESC V` is 90° clockwise). Paper: stays on the **left**. | protocol | **do** |
| 6 | Print position: `ESC $`, `ESC \`, `HT`/`ESC D`, `GS L`, `GS W` | protocol | **do** |
| 7 | `ESC J` feed-dots + `ESC SP` character spacing | protocol | **do** |
| 8 | `GS I` + `tm20 id` | protocol | **do** |
| 9 | `GS ( H` process ID, CLI `--wait` only | protocol | **do** |
| 10 | Slim `StatusRequest` to printer / offline / error / roll | protocol | **do** |
| 11 | Move `raster_from_png` / `graphics` feature out of this crate | split | **do** |
| 12 | Keep 2D that drew. **Delete `Aztec`** — cn=53 is ignored, payload prints as text (`(k5P0tm20`) | protocol | **do** |
| 13 | 1D GS1 DataBar `GS k` `'K'`–`'N'` | protocol | **later** |
| 14 | User-defined characters `ESC &` | protocol | **later** |
| 15 | Page mode | protocol | **later** |
| 16 | Composite symbology | protocol | **never** (unless we need GS1 receipts) |
| 17 | `ESC *`, `GS 8 L`, `ESC d`, NV logos, macros, ASB, `GS ( E`, buzzer, Kanji, typestate | protocol | **never** |
| 18 | Inter rasterizer (proportional, host-side) | host crate | **out** |
| 19 | Gapless CP437 art renderer | host crate | **out** |
| 20 | PNG load / dither / scale to 576 | host crate | **out** |
| 21 | Word wrap / two-column layout helpers | host crate | **out** |

Fanout order for `do`: **1 → 3,4,5 (IR cleanup) → 2 → 6,7 → 8,9,10 → 11.**

---

## Crate split

```
tm20          protocol: Command, encode, Memory/USB/TCP/serial, thin CLI (list, hello, test, debug, status, id)
tm20-host     (name TBD) layout: Inter → 1-bit, PNG, wrap, gapless art ops → Vec<Command>
```

`pack()` (bool pixels → MSB-first bytes) stays in `tm20`. That is the wire format of `Graphics`, not a pretty-printer.

`tm20` CLI stays a protocol exerciser. It does not grow `png`, `art`, or Inter flags.

---

## Protocol: do

### 1. Bitmaps — only `GS ( L`

`ESC *` is columns (dot-matrix comb). `GS v 0` is a row-major fax, marked **OC**; we printed a checkerboard with it. `GS ( L` is the same rows as an object: load fn=112, stamp fn=50.

Keep one IR type. Encode both functions inside `encode`. Callers never see two steps.

```
store:  1D 28 4C pL pH  30 70  30  bx by  31  xL xH yL yH  data
print:  1D 28 4C 02 00 30 32
```

- `x` is **dots**, not bytes (unlike `GS v 0`)
- `bx`,`by` ∈ {1,2}; quadruple is `(2,2)`
- data packing unchanged
- no `GS 8 L` (16-bit length is enough for 576-dot receipts)
- no `ESC *`
- NV/download graphics are a different job (flash/RAM store). Not this.

Paper: reprint checkerboard + one 576-dot strip.

### 2. 1D barcodes — function B only

Function A (`m = 0..6` + NUL) cannot express CODE128. Function B (`m = 'A'..'J'`, length-prefixed) covers the old kinds plus CODE93 `'H'`, CODE128 `'I'`, GS1-128 `'J'`.

CODE128: type carries `{A`/`{B`/`{C`; encode prepends it. Default set B.

`'K'`–`'N'` (1D GS1 DataBar) wait; 2D stacked GS1 is already a different command (`GS ( k`). Do not merge those types.

Paper: CODE128 of `TM20`, CODE93, one GS1-128.

### 3. `Init` only

`Init` and `Reset` are both `ESC @`. Delete `Reset`. Never emit `ESC ? LF 0`.

### 4. No `Font::C`

This unit is `ESC M 0` / `ESC M 1`. C is unrepresentable.

### 5. `Rotate90`

`ESC V` is 90° clockwise in standard mode. 180° is `ESC {` (`UpsideDown`), already present. Rename the variant; keep the command.

**Paper:** `ROTATE` stayed on the **left**. That is the spec. Each glyph is turned 90°; the print head still walks left → right on the original baseline. Not a mirror of upside-down.

### 5b. Upside-down on the right — keep, not a bug

`ESC {` rotates the **whole line** 180°. “Left” in the command stream is the physical **right** of the paper until you flip the slip. After a 180° flip (the point of the mode: kitchen tickets, printer facing the operator), that line is left-aligned again.

The test is `Align::Left` + `UpsideDown(true)` + `UPSIDE DOWN`. Right-hand placement on the unflipped page is intended. Center still centers. To sit on the physical left *without* flipping, use `Align::Right` while upside-down is on (the axes are swapped).

### 5c. Delete Aztec

`encode_aztec` sends `GS ( k` with `cn = 53` (`'5'`), store `fn = 80` (`'P'`), `m = 48` (`'0'`), data `tm20`. Paper showed centered `(k5P0tm20`: that is the store command with `GS` consumed and `pL=7` / `pH=0` (BEL/NUL) invisible. Firmware does not implement Aztec. The IR must not have a variant that can only leak bytes.

Confirm DataMatrix / MaxiCode / PDF417 / stacked GS1 on paper the same way; delete any other `GS ( k` cn that dumps as text. QR we already know draws.

### 6. Position

`HT` / `ESC D` tabs, `ESC $` absolute, `ESC \` relative (signed i16), `GS L` left margin, `GS W` print-area width. `ESC a` stays (per-line justify is a different job).

Paper: left label + absolute-positioned amount.

### 7. `ESC J` + `ESC SP`

`Feed { lines }` stays `n × LF` (hello golden). Add `FeedDots` (`ESC J n`) and `CharSpacing` (`ESC SP n`). Do not add `ESC d`.

### 8. `GS I` / `tm20 id`

Read firmware, serial, name, column mode (`=#0` vs 42-col). Bulk IN. Not a session type.

### 9. `GS ( H` / `--wait`

```
1D 28 48 06 00 30 30 d1 d2 d3 d4   →   37 22 d1 d2 d3 d4 00
```

Opt-in on the CLI. Default remains fire-and-forget. Not inside a normal `Document`.

### 10. Slim status

Keep `DLE EOT` 1–4 (printer, offline, error, roll). Drop ink / peeler / DMD / interface from the public enum. This printer has none of those.

### 11. Strip host from this crate

Delete `raster_from_png` and the `graphics` Cargo feature. PNG belongs next to Inter.

---

## Protocol: keep

Resident style (bold, underline, invert, size, smoothing, double-strike, upside-down), CP437 `Text`, `Raw`, motion units, drawer, cuts, line spacing. QR stays. Other 2D: keep only if the page showed a symbol, not ASCII leakage. Aztec is out.

`Cancel` (`CAN`) stays — it is the page-mode buffer clear, cheap, and we will want it if page mode ever lands.

---

## Protocol: later (not this fanout)

**User-defined characters (`ESC &`).** Overwrite Font A/B cells in a ~12 KB RAM buffer. Height locked (24 dots), width capped near the resident cell (~12). Cleared by `ESC @`. Fine for three custom icons; useless for Inter. Add only if we want CP437-adjacent dingbats.

**Page mode (`ESC L`/`W`/`T`/`FF`).** Second coordinate system. If we add it, the IR is `Page { area, direction, ops }`, not a flag on `Text`.

**1D GS1 DataBar `K`–`N`.** Same encoder as function B; skip until a receipt needs them.

**`GS ( A` / `GS g`.** Firmware self-test and autocut counters. Diagnostics, not receipts.

---

## Protocol: never

| Thing | Why |
| --- | --- |
| `ESC *` | Third bitmap encoding (columns). |
| `GS 8 L` | 32-bit length; receipts do not need it. |
| `ESC d` | Second “feed n lines.” |
| NV graphics / auto logo / watermark | Flash wear; TM Utility. |
| Download graphics fn=83/85 | Second image store. Print-now `GS ( L` is enough. |
| Macros `GS :` | `Document` is the sequence. |
| ASB `GS a` | Unsolicited IN. USB is a pipe. |
| `GS ( E` USB class | Can hide the device from `nusb`. |
| OT-BZ20 / Kanji `FS` | Hardware/ROM we do not have. |
| Fluent builder, hidapi, usbprint, `no_std` | Out of taste. |
| Ready/Busy USB typestate | Printer queues. |
| Composite `GS ( k` cn=52 | Extra 2D family; not needed. |

---

## Host crate: out of `tm20` (do not implement here)

The printer never loads Inter. Inter is a **host rasterizer**: layout at 203 dpi, threshold to 1-bit, emit `Graphics`. That is proportional (kerning baked into pixels). `ESC &` cannot host Inter (12×24 cage).

Same crate as Inter, later:

- PNG → 576-dot 1-bit → `Graphics`
- Word wrap at 48/64 columns; two-column via position commands
- Gapless CP437 art (`ESC 3` = glyph height) — morningprint’s renderer, not Claude/Pi

`tm20` only supplies `Graphics` + `pack` + style commands those helpers lower into.

---

## Sources

- TM-T20III TRG: `sources/tm-t20iii_trg_en_reva.pdf`
- TM-T20 ESC/POS quick reference: http://www.novopos.ch/client/EPSON/TM-T20/TM-T20_eng_qr.pdf
