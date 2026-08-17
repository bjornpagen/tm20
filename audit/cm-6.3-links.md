# Links

- Spec: https://spec.commonmark.org/0.31.2/#links
- Status: **gap**
- Walk: `NodeValue::Link`: inner inlines at italic voice; if dest is empty or equals the visible text, no note (`note_for_dest`); else a `Note::Dest` on the last type span. Link **title** is ignored (`link.title` is never read). Empty inner text still gets a noted empty span.
- Proof: `spec.rs::link_is_italic_with_a_note`, `same_destination_reuses_the_number`, `link_definition_is_not_rendered`; fixture `09-notes.md`. No title, collapsed `[foo][]`, or image-inside-link proof.
- Later do: drop titles forever (closed note is dest-only) or put the title in the note; prove collapsed/full reference links; linked images are `cm-6.4`.
