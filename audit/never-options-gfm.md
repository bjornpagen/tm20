# Options::gfm()

- Spec: comrak kitchen-sink GFM. Not called.
- Status: **never**
- Corpus: `ext-never-b-autolink-off-cases`
- Walk: flags are set one by one in `options()`. `gfm()` would turn on strikethrough, tagfilter, and other extras this crate refuses.
- Proof: `lower.rs` `fn options` (no `gfm()`); `strikethrough_is_plain_text`
- Later do: none
