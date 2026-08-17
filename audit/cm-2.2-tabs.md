# Tabs

- Spec: https://spec.commonmark.org/0.31.2/#tabs
- Status: **unproven**
- Walk: tab-to-space and indented-code column rules are comrak’s. The walk never sees a tab character as a Frame.
- Proof: none
- Later do: one indented-code example that is a tab, not four spaces, still becomes `Frame::Code`.
