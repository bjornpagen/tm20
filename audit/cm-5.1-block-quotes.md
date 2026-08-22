# Block quotes

- Spec: https://spec.commonmark.org/0.31.2/#block-quotes
- Status: **keep**
- Corpus: `cm-5.1-a-quote-basic`, `cm-5.1-b-quote-lazy`, `cm-5.1-c-quote-nested`, `cm-5.1-d-quote-contents`
- Walk: `NodeValue::BlockQuote` → `Frame::Quote` of nested frames. Depth cap 3 (`NEST_CAP`); a fourth nest is `Error::Nesting`.
- Proof: `spec.rs::block_quote`, `quote_nest_cap`; fixture `05-quotes.md`
- Later do: none (cap is the typesetter nest law, not a CM miss)
