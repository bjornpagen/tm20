# tm20

Markdown → thermal tape for the Epson TM-T20III: 576 dots, 80 mm, 203 dpi.
A strict printable subset of CommonMark and GFM. Unsupported constructs,
missing glyphs, and clipped content are errors with source context—not fallbacks.
Diagnostics include error codes, source lines/character columns, excerpts, and
repair guidance. Code and tests define behavior.

Rust 1.91+; stable toolchain. Install both commands with
`cargo install tm20-cli --locked`.

| Cargo dependency | Use |
| --- | --- |
| `cargo add tm20` | Typed ESC/POS and USB/serial/TCP transport |
| `cargo add tm20-md tm20-set tm20` | Markdown → portable fonts → ESC/POS; [complete example](https://github.com/bjornpagen/tm20/blob/main/crates/tm20-md/examples/markdown.rs) |

```sh
tm20-set --dry print md receipt.md   # Validate without a printer
tm20-set print md receipt.md         # Print over USB
tm20-set --output receipt.bin print md receipt.md  # Encode without delivery
```

For libraries, `FaceTable::portable()` supplies the CLI's embedded font profile;
reuse it across documents. `tm20-set`'s default `portable-fonts` feature can be
disabled when supplying your own fonts. `tm20_md::image_bytes` reads local files
only (not a path sandbox); a custom loader owns any network policy.
Errors are non-exhaustive typed enums: use their fields/codes, not parsed prose.

| Feature | Supported behavior | Specification |
| --- | --- | --- |
| Paragraphs | 11 pt selected sans face; word wrapping, no hyphenation | [CommonMark](https://spec.commonmark.org/0.31.2/#paragraphs) |
| Emphasis / strong | Oblique / bold; combinations use bold oblique | [CommonMark](https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis) |
| Headings | H1: 18 pt; H2–H6: 11 pt bold. Plain text only; empty or styled headings reject | [ATX](https://spec.commonmark.org/0.31.2/#atx-headings), [Setext](https://spec.commonmark.org/0.31.2/#setext-headings) |
| Soft / hard breaks | Soft breaks become spaces; hard breaks start a new line | [Soft](https://spec.commonmark.org/0.31.2/#soft-line-breaks), [hard](https://spec.commonmark.org/0.31.2/#hard-line-breaks) |
| Escapes / entities | Decoded text; glyph coverage depends on supplied fonts | [Escapes](https://spec.commonmark.org/0.31.2/#backslash-escapes), [entities](https://spec.commonmark.org/0.31.2/#entity-and-numeric-character-references) |
| Code spans | Selected monospace; normalized whitespace; fitting spans stay unbroken | [CommonMark](https://spec.commonmark.org/0.31.2/#code-spans) |
| Code blocks | Fenced or indented; selected monospace, no highlighting or wrapping; overwide lines reject | [Fenced](https://spec.commonmark.org/0.31.2/#fenced-code-blocks), [indented](https://spec.commonmark.org/0.31.2/#indented-code-blocks) |
| Lists | Dash bullets; ordered starts and delimiters preserved; tight/loose spacing; nesting ≤3 | [CommonMark](https://spec.commonmark.org/0.31.2/#lists) |
| Task lists | Checked and unchecked boxes | [GFM](https://github.github.com/gfm/#task-list-items-extension-) |
| Block quotes | Indented; nesting ≤3, independent of list depth | [CommonMark](https://spec.commonmark.org/0.31.2/#block-quotes) |
| Thematic breaks | Two-dot rule across the full tape, including inside containers | [CommonMark](https://spec.commonmark.org/0.31.2/#thematic-breaks) |
| Tables | Two/three columns only; header-only tables accepted; ragged rows reject | [GFM](https://github.github.com/gfm/#tables-extension-) (restricted) |
| Table alignment | Left/right; centered columns reject. Right-aligned cells use tabular digits; clipped content rejects | [GFM](https://github.github.com/gfm/#tables-extension-) (restricted) |
| Table pipes | Literal pipes must be escaped, including inside code spans | [GFM](https://github.github.com/gfm/#tables-extension-) |
| Links | Italic labels; numbered destination endnotes when nonredundant; destinations deduplicate; undefined references and conflicting titles reject | [CommonMark](https://spec.commonmark.org/0.31.2/#links) |
| Autolinks | Angle links, recognized bare URLs and email; normally no redundant endnote | [CommonMark](https://spec.commonmark.org/0.31.2/#autolinks), [GFM](https://github.github.com/gfm/#autolinks-extension-) |
| Images | Standalone PNG/JPEG; local files by default, HTTP(S) only with `--allow-remote-images`; shrink to local width, dither to monochrome; no printed alt text | [CommonMark](https://spec.commonmark.org/0.31.2/#images) (restricted) |
| Raw HTML | Rejected, including comments; escaped HTML and code literals remain text | [CommonMark](https://spec.commonmark.org/0.31.2/#raw-html), [HTML blocks](https://spec.commonmark.org/0.31.2/#html-blocks) (unsupported) |
| Strikethrough | Strike line across text, including nested styles and wrapped lines | [GFM](https://github.github.com/gfm/#strikethrough-extension-) |
| Footnotes | Named references and multiblock definitions; first-use numbering shared with link notes; unused omitted, undefined references reject | Extension outside CommonMark/GFM |
| Math | LaTeX via RaTeX: `\(inline\)`, `\[display\]`; display math outside inline styling/tables; dollar delimiters disabled; embedded KaTeX glyphs only; unsupported formulas and host-font fallback reject | Extension outside CommonMark/GFM |
| Smart punctuation | Curly quotes, en/em dashes, ellipses in prose | Extension outside CommonMark/GFM |
| Other extensions | No YAML metadata, CSS, definition lists, or image-size attributes | Not enabled |

| Crate | Responsibility |
| --- | --- |
| [tm20](https://docs.rs/tm20) | ESC/POS encoding, raster bands, barcodes/symbols, USB/serial/TCP/memory transports. CODE128-C encoding and DataMatrix model support remain unverified |
| [tm20-set](https://docs.rs/tm20-set) | Typed sheets, shared parsed fonts, measured layout, rasterization, lossless banding, PNG previews |
| [tm20-md](https://docs.rs/tm20-md) | Strict Markdown parsing, source diagnostics, math and local image loading |
| [tm20-cli](https://crates.io/crates/tm20-cli) | `tm20-set` typesetter and `tm20` protocol executables; typed [usage-rs](https://github.com/jdx/usage) parsing/help/specs; complete batch prepared before delivery |

| Image network policy | Behavior |
| --- | --- |
| Permission | Denied before network access in every output mode. `--allow-remote-images` opts in, including with `--dry` |
| Proxy | Rust `reqwest`; `HTTP_PROXY`/`HTTPS_PROXY`, then `ALL_PROXY` (lowercase also accepted; uppercase wins). `NO_PROXY`/`no_proxy` controls bypasses |
| System settings | macOS manual HTTP(S) proxies when no explicit environment proxy applies. Linux uses proxy environment variables, not GNOME/KDE settings. For consistent bypass rules, set `NO_PROXY`; macOS system exception lists are not imported |
| Unsupported configuration | macOS PAC/WPAD or system SOCKS settings require an explicit proxy environment variable; lookup/configuration failures error. `socks5h://` resolves destination names through the proxy |
| Boundaries | Proxy policy checked at each redirect; HTTP(S) only, ≤10 redirects, no direct retry after proxy failure. OS controls VPN/WireGuard routing; no tunnel management or kill-switch guarantees |

Both CLIs are foreground, one-shot processes. Options precede the command.
`--help`, `--version`, and `__usage_spec__` are device-free; no external Usage
executable is needed to parse arguments.

| CLI | Behavior |
| --- | --- |
| `tm20-set print md FILE\|DIR\|-` | UTF-8 file, immediate lowercase `*.md` entries sorted by path, or stdin. One cut per document; stdin output stem is `stdin` |
| `tm20-set print [ticket\|prose\|helvetica\|suite\|all]` | Built-in sheets; bare `print` lists them. `helvetica` is the legacy specimen name and uses the selected profile |
| `--usb` / `--usb-serial ID` | USB delivery by default; select a TM-T20III by its USB serial number. `--serial ID` is an alias, **not** a serial-port path |
| `--tcp HOST:PORT` | Raw TCP delivery, usually port 9100; IPv6 uses `[ADDRESS]:PORT`. Not HTTP: image proxy settings do not apply |
| `--serial-port PATH --baud RATE` | Direct serial-port delivery; both required, nonzero rate; 8 data bits, no parity, one stop bit, no flow control |
| `--dry` | Validate/encode only; no printer. Byte counts and diagnostics go to stderr |
| `--output FILE\|-` | Concatenated ESC/POS bytes to a file (replaced after successful preparation) or stdout; no printer |
| `--fake-delivery DIR` | One encoded `DIR/<name>.bin` per job; no printer |
| `--png DIR` | Also write `DIR/<name>.png` at 2×; replaces existing previews. Alone this still prints |
| Output conflicts | `--dry`, `--output`, and `--fake-delivery` are exclusive and reject explicit printer destinations. USB, TCP, and serial-port destinations are exclusive |
| `--base-dir DIR` | Relative image root for stdin only; otherwise stdin uses the working directory. Files always resolve images beside the source |
| `--fonts portable\|macos` | Portable is default on every OS: embedded Source Sans 3 (regular/italic/bold/bold italic/light) and Source Code Pro. Changes typography from the previous default. `macos` explicitly loads Helvetica/Menlo from `/System/Library/Fonts`; no fallback |
| `--allow-remote-images` | Explicit permission to fetch HTTP(S) images; default deny in every mode |
| `--failure-exit-code CODE` | Both CLIs: 1–255 for preparation/delivery/configuration failures after successful argument parsing (default 1). Syntax errors return 1; success/help return 0. Signals retain normal OS termination semantics |
| `tm20` | Low-level `list`, `debug`, `hello`, `text`, `ruler`, `status`, `recover`, `id`, `qr`, `ean13`, `test [ID\|all]`. Same transport flags; `list`/`debug` are USB-only. `--dry` shows planned hex, except `list`/`debug` reject. `--wait` waits for job completion |
| Font licensing | Unmodified [Source Sans 3 3.052R](https://github.com/adobe-fonts/source-sans/tree/3.052R) and [Source Code Pro 2.042R](https://github.com/adobe-fonts/source-code-pro/tree/d3f1a5962cde503f9409c21e58527611d4a19ef1), SIL OFL 1.1. Notices are included and accessible with `tm20-set font-licenses` |

| Runtime | Requirements / boundary |
| --- | --- |
| macOS | Native executable; portable fonts need no installation. USB uses nusb |
| Fedora | Native GNU/Linux or static musl executable; USB needs access to `/dev/bus/usb` and `/sys/bus/usb/devices`. No libudev discovery dependency |
| gokrazy | Static musl ARM64/x86-64 executable, included in the OS image by its operator. USB needs usbfs/sysfs and a supported host controller. No Go launcher or libc installation |
| USB ownership | nusb detaches a kernel driver from the selected interface when claiming it, then attempts reattachment on normal release. Permissions and competing CUPS/usblp users are operator concerns |
| Supervision | systemd, launchd, s6, or gokrazy owns scheduling, environment, logs, and restarts. No daemon mode, queue, watcher, service files, retries, or CLI timeouts |
| Delivery failure | May be partial; never automatically resent. A supervisor restart can print duplicates. gokrazy stops supervision on 0 or 125; `--failure-exit-code 125` handles a valid job's failure, not signals, power loss, or syntax errors |
| Files / HTTPS | Choose writable output paths (`/perm` or `/tmp` on gokrazy). Remote HTTPS needs DNS, a trusted clock, and CA certificates; gokrazy's `/etc/ssl/ca-bundle.pem` is recognized. `SSL_CERT_FILE` can select a custom bundle |

Build natively with `cargo build --release --locked -p tm20-cli --bins`.
For self-contained Linux binaries, install [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild)
and Zig, add the Rust musl targets, then:
`cargo zigbuild --release --locked -p tm20-cli --bins --target aarch64-unknown-linux-musl --target x86_64-unknown-linux-musl`.
Outputs are under `target/<target>/release/`. These build checks do not establish
hardware compatibility; OS packaging and Linux test suites are outside this repository's current scope.

Agent workflow: [SKILL.md](https://github.com/bjornpagen/tm20/blob/main/.claude/skills/tm20/SKILL.md).

Device-free parallel tests: `cargo nextest run --workspace --locked`.
Doctests: `cargo test --workspace --locked --doc`. Each visual fixture is an independent test.
