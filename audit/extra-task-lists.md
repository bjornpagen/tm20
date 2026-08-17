# Task lists

- Spec: not CommonMark; `options.extension.tasklist`
- Status: **keep**
- Walk: `NodeValue::TaskItem` → `ItemMark::Task { checked: t.symbol.is_some() }` inside `Frame::List`. Nesting uses the list cap.
- Proof: `spec.rs::task_list_items`, `nested_task_list`; fixture `07-tasks.md`
- Later do: none
