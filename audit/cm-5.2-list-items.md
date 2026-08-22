# List items

- Spec: https://spec.commonmark.org/0.31.2/#list-items
- Status: **keep**
- Corpus: `cm-5.2-a-item-indent`, `cm-5.2-b-item-blocks`, `cm-5.2-c-item-empty`, `cm-5.2-d-item-heading`
- Walk: each `Item` / `TaskItem` becomes a `ListItem` whose `frames` are `blocks` of the item (nested lists, paragraphs). Bullet markers `*`, `-`, `+` all become `Marker::Dash` (en dash in compose). Ordered start and `.` / `)` delim are kept.
- Proof: goldens `cm-5.2-a`…`d`, `cm-5.3-a-markers` (`*` / `+` are dashes); `spec.rs` list facts; fixture `06-lists.md`.
- Later do: none.
