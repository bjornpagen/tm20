# Strikethrough

- Spec: GFM, not CommonMark. `options.extension.strikethrough` is off.
- Status: **never**
- Walk: `~~no~~` is ordinary text. If the extension were on, `NodeValue` would hit `inline`’s `_ => Error::Html`.
- Proof: `spec.rs::strikethrough_is_plain_text`
- Later do: none (`Cut` has no strike)
