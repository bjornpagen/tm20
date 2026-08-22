# Links

- Spec: https://spec.commonmark.org/0.31.2/#links
- Status: **keep**
- Corpus: `cm-6.3-a-inline-links`, `cm-6.3-b-link-nesting`, `cm-6.3-c-link-notes`, `cm-6.3-d-brackets`
- Walk: `NodeValue::Link`: inner inlines at italic voice. `note_for_dest` allocates a numbered `Note::Dest` unless dest is empty or equals the visible text (after stripping `mailto:` / a comrak-added `http(s)://`). A non-empty title is stored on the note and prints above the URL. Empty inner text still gets a noted empty span.
- Proof: goldens `cm-6.3-a`…`d`, `cm-4.7-a` (reference links and titles); `spec.rs` link facts; fixture `09-notes.md`.
- Later do: none. Linked images stay `cm-6.4` (`Error::MixedImage` if not a sole-child paragraph).
