# Container blocks and leaf blocks

- Spec: https://spec.commonmark.org/0.31.2/#container-blocks-and-leaf-blocks
- Status: **keep**
- Walk: `Cx::blocks` walks children. Containers become `Frame::Quote` / `Frame::List` (and extra `Frame::Cols`). Leaves become `Text` / `Mark` / `Head` / `Code` / `Rule` / `Figure` / `Math`, or `Error::Html`.
- Proof: the `spec.rs` block tests as a set; `tm20-set` `Frame` is closed
- Later do: none
