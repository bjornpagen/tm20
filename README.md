# tm20

Markdown → thermal tape for the Epson TM-T20III: 576 dots, 80 mm, 203 dpi.
A strict printable subset of CommonMark and GFM. Unsupported constructs,
missing glyphs, and clipped content are errors with source context—not fallbacks.
Diagnostics include error codes, source lines/character columns, excerpts, and
repair guidance. Code and tests define behavior.

| Feature | Supported behavior | Specification |
| --- | --- | --- |
| Paragraphs | 11 pt Helvetica; word wrapping, no hyphenation | [CommonMark](https://spec.commonmark.org/0.31.2/#paragraphs) |
| Emphasis / strong | Oblique / bold; combinations use bold oblique | [CommonMark](https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis) |
| Headings | H1: 18 pt; H2–H6: 11 pt bold. Plain text only; empty or styled headings reject | [ATX](https://spec.commonmark.org/0.31.2/#atx-headings), [Setext](https://spec.commonmark.org/0.31.2/#setext-headings) |
| Soft / hard breaks | Soft breaks become spaces; hard breaks start a new line | [Soft](https://spec.commonmark.org/0.31.2/#soft-line-breaks), [hard](https://spec.commonmark.org/0.31.2/#hard-line-breaks) |
| Escapes / entities | Decoded text; glyph coverage depends on supplied fonts | [Escapes](https://spec.commonmark.org/0.31.2/#backslash-escapes), [entities](https://spec.commonmark.org/0.31.2/#entity-and-numeric-character-references) |
| Code spans | Menlo; normalized whitespace; fitting spans stay unbroken | [CommonMark](https://spec.commonmark.org/0.31.2/#code-spans) |
| Code blocks | Fenced or indented; Menlo, no highlighting or wrapping; overwide lines reject | [Fenced](https://spec.commonmark.org/0.31.2/#fenced-code-blocks), [indented](https://spec.commonmark.org/0.31.2/#indented-code-blocks) |
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
| Math | LaTeX via RaTeX: `\(inline\)`, `\[display\]`; display math outside inline styling/tables; dollar delimiters disabled; unsupported formulas reject | Extension outside CommonMark/GFM |
| Smart punctuation | Curly quotes, en/em dashes, ellipses in prose | Extension outside CommonMark/GFM |
| Other extensions | No YAML metadata, CSS, definition lists, or image-size attributes | Not enabled |

| Crate | Responsibility |
| --- | --- |
| [tm20](crates/tm20/src/lib.rs) | ESC/POS encoding, raster bands, barcodes/symbols, USB/serial/TCP/memory transports. CODE128-C encoding and DataMatrix model support remain unverified |
| [tm20-set](crates/tm20-set/src/lib.rs) | Typed sheets, shared parsed fonts, measured layout, rasterization, lossless banding, PNG previews |
| [tm20-md](crates/tm20-md/src/lib.rs) | Strict Markdown parsing, source diagnostics, math and local image loading |
| [tm20-cli](crates/tm20-cli/src/main.rs) | `tm20-set` executable: full batch preparation before USB opens; device-free `--dry` previews; opt-in HTTP(S) images; macOS Helvetica/Menlo fonts |

| Image network policy | Behavior |
| --- | --- |
| Permission | Denied before network access in every output mode. Leading `--allow-remote-images` opts in, including with `--dry` |
| Proxy | Rust `reqwest`; `HTTP_PROXY`/`HTTPS_PROXY`, then `ALL_PROXY` (lowercase also accepted; uppercase wins). `NO_PROXY`/`no_proxy` controls bypasses |
| System settings | macOS manual HTTP(S) proxies when no explicit environment proxy applies. Linux uses proxy environment variables, not GNOME/KDE settings. For consistent bypass rules, set `NO_PROXY`; macOS system exception lists are not imported |
| Unsupported configuration | macOS PAC/WPAD or system SOCKS settings require an explicit proxy environment variable; lookup/configuration failures error. `socks5h://` resolves destination names through the proxy |
| Boundaries | Proxy policy checked at each redirect; HTTP(S) only, ≤10 redirects, no direct retry after proxy failure. OS controls VPN/WireGuard routing; no tunnel management or kill-switch guarantees |

Agent workflow: [SKILL.md](.claude/skills/tm20/SKILL.md).

Device-free parallel tests: `cargo nextest run --workspace --locked`.
Doctests: `cargo test --workspace --locked --doc`. Each visual fixture is an independent test.
