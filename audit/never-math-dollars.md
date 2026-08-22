# Math dollars

- Spec: not CommonMark. `options.extension.math_dollars` is off.
- Status: **never**
- Corpus: `ext-math-f-dollars`
- Walk: `$4.50` is text. Enabling dollars would steal currency.
- Proof: `spec.rs::dollars_are_currency`; fixture `12-math.md`
- Later do: none
