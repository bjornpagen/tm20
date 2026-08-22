# Link reference definitions

- Spec: https://spec.commonmark.org/0.31.2/#link-reference-definitions
- Status: **keep**
- Corpus: `cm-4.7-a-ref-links`, `cm-4.7-b1-dup-unused`, `cm-4.7-b2-only-defs`
- Walk: comrak consumes the definition. It is not a `NodeValue` the walk renders. Shortcut `[foo]` after `[foo]: /url` becomes a `Link` and follows `cm-6.3`.
- Proof: `spec.rs::link_definition_is_not_rendered`
- Later do: collapsed `[foo][]` and full `[foo][bar]` proofs.
