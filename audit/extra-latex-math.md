# LaTeX math

- Spec: not CommonMark; `options.extension.math_latex` only (`\(inline\)`, `\[display\]`)
- Status: **keep**
- Walk: inline `Math` → `Span::Math` via RaTeX. Display math flushes the paragraph and becomes `Frame::Math`. Bad TeX is `Error::Math`. Math in a heading is `Error::Math`. Dollars are not this extension (`never-math-dollars.md`).
- Proof: `spec.rs::inline_math_is_a_raster_box`, `display_math_breaks_the_paragraph`, `bad_latex_is_an_error`, `math_in_a_heading_is_an_error`; fixture `12-math.md`
- Later do: none
