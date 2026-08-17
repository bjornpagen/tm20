# Thematic breaks

- Spec: https://spec.commonmark.org/0.31.2/#thematic-breaks
- Status: **keep**
- Walk: `NodeValue::ThematicBreak` → `Frame::Rule` with `Thickness::Two`. `---`, `***`, and `___` are not distinguished.
- Proof: `spec.rs::thematic_break`; fixture `03-rule.md`
- Later do: none unless a later pass wants `*` / `_` as `Thickness::One`.
