# Autolinks

- Spec: https://spec.commonmark.org/0.31.2/#autolinks
- Status: **keep**
- Corpus: `cm-6.5-a-autolinks`, `cm-6.5-b-gfm-autolink`
- Walk: CM autolinks are `NodeValue::Link`. URI form dest equals text → italic, no note. `mailto:` is stripped before the dest==text compare, so `<a@b.com>` is italic without a note. Bare GFM `www.…` dest is `http://` plus the text; that prefix is stripped the same way.
- Proof: goldens `cm-6.5-a-autolinks`, `cm-6.5-b-gfm-autolink`; `spec.rs::autolink_has_no_note`.
- Later do: none.
