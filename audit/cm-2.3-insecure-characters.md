# Insecure characters

- Spec: https://spec.commonmark.org/0.31.2/#insecure-characters
- Status: **unproven**
- Walk: NUL stripping is comrak’s. The walk has no `Error` arm for U+0000.
- Proof: none
- Later do: a NUL in a paragraph still lowers to text without a panic.
