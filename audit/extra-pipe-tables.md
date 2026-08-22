# Pipe tables

- Spec: not CommonMark; GFM tables via `options.extension.table`
- Status: **keep**
- Corpus: `ext-table-a-two-col`, `ext-table-b-three-col`, `ext-table-c-squeeze`, `ext-table-d-overflow`, `ext-table-e-cell-content`, `ext-table-f-pipes`, `ext-table-g-degenerate`, `ext-table-h-numeric`, `rej-table-a-one-col`, `rej-table-b-four-col`
- Walk: `NodeValue::Table` → `Frame::Cols`. Only 2 or 3 columns (`Error::Cols` otherwise). `TableAlignment::Right` → `ColAlign::End`; **center and left both `ColAlign::Start`**. Header row forced `Cut::Bold`.
- Proof: goldens `ext-table-a`…`h`; rejects `rej-table-a`, `rej-table-b`. A ragged row is not a reject — comrak pads to the delimiter width. Center alignment is Start; that is the dialect.
- Later do: none unless house style asks for a decimal column.
