# GFM autolink

- Spec: not CommonMark; `options.extension.autolink` (bare `https://…` in prose)
- Status: **keep**
- Walk: same `Link` path as CM autolinks. Bare URL dest equals visible text → italic, no note.
- Proof: `spec.rs::bare_autolink_is_italic_without_a_note`
- Later do: none (CM angle-bracket autolinks are `cm-6.5`)
