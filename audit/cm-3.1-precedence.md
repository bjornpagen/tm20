# Precedence

- Spec: https://spec.commonmark.org/0.31.2/#precedence
- Status: **unproven**
- Walk: block vs inline precedence is entirely comrak. The walk consumes the AST it is given.
- Proof: none dedicated (list/quote/code tests assume comrak’s choices)
- Later do: one fixture where a list marker in a paragraph stays text, matching the spec example.
