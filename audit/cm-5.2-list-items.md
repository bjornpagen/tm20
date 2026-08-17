# List items

- Spec: https://spec.commonmark.org/0.31.2/#list-items
- Status: **keep**
- Walk: each `Item` / `TaskItem` becomes a `ListItem` whose `frames` are `blocks` of the item (nested lists, paragraphs). Bullet markers `*`, `-`, `+` all become `Marker::Dash` (en dash in compose). Ordered start and `.` / `)` delim are kept.
- Proof: `spec.rs::bullet_and_ordered_lists`, `nested_list_item_blocks`; fixture `06-lists.md`. `*` and `+` bullets unproven.
- Later do: `*` and `+` still `Marker::Dash`.
