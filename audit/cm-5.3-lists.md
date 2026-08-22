# Lists

- Spec: https://spec.commonmark.org/0.31.2/#lists
- Status: **keep**
- Corpus: `cm-5.3-a-markers`, `cm-5.3-b-ordered-start`, `cm-5.3-c-tight-loose`, `cm-5.3-d-interrupt`, `cm-5.3-e-nested-mixed`, `cm-5.3-f-runover`
- Walk: `NodeValue::List` → `Frame::List` with `ListFit::Tight` / `Loose` from `nl.tight`. Nest cap 3 → `Error::Nesting`. A paragraph between lists splits them (`a_paragraph_breaks_lists`).
- Proof: `spec.rs::loose_list`, `blank_between_same_type_items_is_one_loose_list`, `a_paragraph_breaks_lists`, `list_nest_cap`; fixture `06-lists.md`
- Later do: none
