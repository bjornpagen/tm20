# Autolinks

- Spec: https://spec.commonmark.org/0.31.2/#autolinks
- Status: **gap**
- Walk: CM autolinks are `NodeValue::Link`. URI form `<https://example.com>` has dest equal to text → italic, no note (`autolink_has_no_note`). Email form `<foo@bar.com>` typically has dest `mailto:foo@bar.com` ≠ text, so `note_for_dest` would allocate a destination note. That case is unproven and likely unwanted.
- Proof: `spec.rs::autolink_has_no_note` only
- Later do: treat `mailto:` dest as autolink (no note) if dest is `mailto:` plus the visible text.
