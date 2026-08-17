# Pipe tables

- Spec: not CommonMark; GFM tables via `options.extension.table`
- Status: **gap**
- Walk: `NodeValue::Table` → `Frame::Cols`. Only 2 or 3 columns (`Error::Cols` otherwise). `TableAlignment::Right` → `ColAlign::End`; **center and left both `ColAlign::Start`**. Header row forced `Cut::Bold`.
- Proof: `spec.rs::pipe_table`, `one_column_table_is_an_error`; fixture `11-tables.md`. No three-column or center-align proof in markdown tests (three-column exists in `tm20-set` algebra).
- Later do: map center to a closed align if `tm20-set` grows one; otherwise document center as start.
