# Entity and numeric character references

- Spec: https://spec.commonmark.org/0.31.2/#entity-and-numeric-character-references
- Status: **keep**
- Corpus: `cm-2.5-a-entities`
- Walk: comrak decodes before the walk. Named `&amp;` is proven. Numeric `&#65;` / `&#x41;` take the same `NodeValue::Text` path and are not separately tested.
- Proof: `spec.rs::backslash_and_entity` (`&amp;` → `&`)
- Later do: one numeric and one hex entity in `spec.rs`.
