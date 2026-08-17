# ATX headings

- Spec: https://spec.commonmark.org/0.31.2/#atx-headings
- Status: **gap**
- Walk: `heading`: math in a heading is `Error::Math`. Inlines flatten to a string (`flatten`). Level `<= 1` → `Frame::Mark` at 18 pt. Any other level → `Frame::Head` at 11 pt. Levels 3–6 are the same Head as level 2. Closing hashes are stripped by comrak.
- Proof: `spec.rs::atx_and_setext`, `heading_inlines_flatten`, `math_in_a_heading_is_an_error`; fixture `02-heads.md`. No h3–h6 proof.
- Later do: either document collapse as never, or give h3–h6 a representable size.
