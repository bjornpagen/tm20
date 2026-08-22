# Tabs

- Spec: https://spec.commonmark.org/0.31.2/#tabs
- Status: **keep**
- Corpus: `cm-2.2-a-tabs-code`, `cm-2.2-b-tabs-lists`
- Walk: tab-to-space and indented-code column rules are comrak’s. A tab that survives into a fence is parsed out in `Code::new` (`detab`, stop every 8) so Menlo never sees U+0009.
- Proof: goldens `cm-2.2-a-tabs-code`, `cm-2.2-b-tabs-lists`.
- Later do: none.
