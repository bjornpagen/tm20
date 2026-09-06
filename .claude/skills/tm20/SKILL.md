---
name: tm20
description: Author, preview, or print Markdown receipts and tapes with this repository's Epson TM-T20III typesetter. Use for thermal-printer output, not generic Markdown editing.
---

# Author and print a tape

Run from this workspace root. Read [README.md](../../../README.md) for the
supported features and deviations when needed; do not load the corpus
or vendor manuals for an ordinary print.

## Choose the requested effect

- **Print requested:** print directly; do not add an unrequested preview or
  confirmation step. That request authorizes the specified job, not a test
  catalog, device reconfiguration, or repeat copies.
- **Preview requested:** use `--dry --png DIR`, then inspect the PNG.
  `--png` alone also prints.
- **Write/edit only:** create the Markdown; do not print implicitly.
  `--dry` alone validates and encodes without writing previews or opening USB.

Author new tapes and figures in a unique temporary directory under `/tmp`,
not in the repo. Keep supplied files and existing fixtures in place.

```sh
# Print one file; --serial S may precede print to select a device.
cargo run --locked --bin tm20-set -- print md /tmp/TAPE_DIR/tape.md

# No USB; writes PREVIEW_DIR/tape.png at 2×.
cargo run --locked --bin tm20-set -- --dry --png /tmp/PREVIEW_DIR print md /tmp/TAPE_DIR/tape.md
```

Replace placeholders with actual paths. Options precede `print`. A directory
prints immediate lowercase `*.md` entries sorted by path, one cut each;
select a directory only when the user wants the batch. Preview also works
for directories. Same-named PNGs are overwritten. Built-ins: `ticket`,
`prose`, `helvetica`, `suite`; bare `print` lists them.

## Write for 576 dots

- Prefer one short `#` masthead. `##`–`######` are all the same bold body
  size. Headings must be nonempty plain text. Use short paragraphs with one
  blank line between blocks; extra blank lines add no space. A trailing
  backslash gives address-style hard breaks.
- Emphasis, strong, `~~strikethrough~~`, code, lists, tasks, and quotes work.
  Nest lists/quotes at most three deep. Code blocks never wrap: split long
  lines explicitly. Missing glyphs and clipped content reject.
- Tables have two/three columns. Use `---:` for numbers with fixed decimals;
  `:---:` rejects. Every row must have the same cell count. A rule then
  header-only total table is the receipt idiom. Literal cell pipes need
  `\|`, including inside code spans.
- Labeled links make destination endnotes; duplicate destinations need
  consistent titles. Bare URLs normally need no note. Footnotes share their
  number sequence; unused definitions disappear, undefined references reject.
  Keep links and styled text out of headings.
- Math: `\(inline\)` or `\[display\]`; dollars are currency. No math in
  headings; display math must be outside inline styling, links, and tables.
  Escape literal brackets; bare brackets can be reference syntax.
- PNG/JPEG images stand alone in a paragraph. Relative paths resolve beside
  the Markdown. HTTP(S) images are denied by default; add the leading
  `--allow-remote-images` only when the user intends network access, never
  just to silence a validation error. With that opt-in, `--dry` also fetches.
  Images shrink to local width, never upscale, and print without alt text.
  Prefer high contrast.
- No raw HTML/comments, YAML front matter, or CSS. Do not
  paste GitHub badges or HTML layout. `---` immediately under text can
  be a Setext heading; surround intended rules with blank lines.

## Failure boundaries

The complete batch encodes before USB opens: parsing/rendering errors send
nothing. Read the error code, filename, source line/character column, excerpt,
and reason. Correct that source construct; `--dry` checks a repair without
printing. Preserve the intended content; do not weaken validation,
substitute fonts, or silently drop rejected material.

Image fetching uses standard proxy environment variables and macOS manual
HTTP(S) proxies; Linux uses environment variables. For proxy bypasses use
`NO_PROXY`. Unsupported macOS automatic/SOCKS settings need an explicit
proxy URL (`socks5h://` for proxy-side DNS). Do not repair network errors by
clearing proxies, disabling TLS checks, or changing VPN/system routing.

After a write/completion failure, delivery may be partial: never resend
automatically; establish what printed or ask before another copy. `hello`,
`test all`, status, and debug are device operations, not harmless validation.

The CLI needs Helvetica.ttc and Menlo.ttc in macOS `/System/Library/Fonts`;
real printing needs the TM-T20III (`04b8:0e28`). Missing fonts are a prerequisite
failure. Keep disposable tapes and generated output out of commits.
