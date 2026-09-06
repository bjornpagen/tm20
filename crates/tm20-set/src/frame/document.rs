//! Sheet, notes, and checked admission. [`SheetBuilder`] is the sole numbering authority.

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, FaceRequirements};
use crate::frame::blocks::{ColBody, Frame, ItemBody};
use crate::frame::inline::{NoteId, Span};
use crate::geometry::Measure;

/// Source list/quote cap. The notes apparatus is not an extra source list.
const NEST_CAP: u8 = 3;

/// One slot in the sheet’s note apparatus. Links, captions, and footnotes share the numbers.
pub enum Note<'a> {
    Dest {
        dest: Cow<'a, str>,
        title: Option<Cow<'a, str>>,
        location: Option<crate::SourceLocation>,
    },
    Blocks(Vec<Frame<'a>>),
}

impl<'a> Note<'a> {
    pub fn dest(dest: impl Into<Cow<'a, str>>) -> Self {
        Self::Dest {
            dest: dest.into(),
            title: None,
            location: None,
        }
    }
}

enum Slot<'a> {
    Reserved,
    Defined(Note<'a>),
}

/// Forward-reference note allocator. Finish never emits a partially built sheet.
pub struct SheetBuilder<'a> {
    width: Measure,
    frames: Vec<Frame<'a>>,
    slots: Vec<Slot<'a>>,
}

impl<'a> SheetBuilder<'a> {
    pub fn new(width: Measure) -> Self {
        Self {
            width,
            frames: Vec::new(),
            slots: Vec::new(),
        }
    }

    pub fn reserve_note(&mut self) -> Result<NoteId, Error> {
        let n = u32::try_from(self.slots.len() + 1).map_err(|_| Error::InvalidNote {
            number: 0,
            count: self.slots.len(),
        })?;
        let id = NoteId::from_index(NonZeroU32::new(n).ok_or(Error::InvalidNote {
            number: 0,
            count: self.slots.len(),
        })?);
        self.slots.push(Slot::Reserved);
        Ok(id)
    }

    pub fn define_note(&mut self, id: NoteId, note: Note<'a>) -> Result<(), Error> {
        let idx = usize::try_from(id.get_u32() - 1).unwrap_or(usize::MAX);
        match self.slots.get_mut(idx) {
            Some(Slot::Reserved) => {
                self.slots[idx] = Slot::Defined(note);
                Ok(())
            }
            Some(Slot::Defined(_)) => Err(Error::InvalidNote {
                number: id.get_u32(),
                count: self.slots.len(),
            }),
            None => Err(Error::InvalidNote {
                number: id.get_u32(),
                count: self.slots.len(),
            }),
        }
    }

    pub fn push_frame(&mut self, frame: Frame<'a>) {
        self.frames.push(frame);
    }

    pub fn finish(self) -> Result<Sheet<'a>, Error> {
        let count = self.slots.len();
        let mut notes = Vec::with_capacity(count);
        for (i, slot) in self.slots.into_iter().enumerate() {
            match slot {
                Slot::Defined(note) => notes.push(note),
                Slot::Reserved => {
                    return Err(Error::InvalidNote {
                        number: (i + 1) as u32,
                        count,
                    });
                }
            }
        }
        Ok(Sheet {
            width: self.width,
            frames: self.frames,
            notes,
        })
    }
}

/// Authoring document. Compiles after [`Sheet::admit`].
pub struct Sheet<'a> {
    pub width: Measure,
    pub frames: Vec<Frame<'a>>,
    notes: Vec<Note<'a>>,
}

impl<'a> Sheet<'a> {
    pub fn tape(frames: Vec<Frame<'a>>) -> Self {
        Self {
            width: Measure::TAPE,
            frames,
            notes: Vec::new(),
        }
    }

    pub fn add_note(&mut self, note: Note<'a>) -> Result<NoteId, Error> {
        let n = u32::try_from(self.notes.len() + 1).map_err(|_| Error::InvalidNote {
            number: 0,
            count: self.notes.len(),
        })?;
        let id = NoteId::from_index(NonZeroU32::new(n).ok_or(Error::InvalidNote {
            number: 0,
            count: self.notes.len(),
        })?);
        self.notes.push(note);
        Ok(id)
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn notes(&self) -> &[Note<'a>] {
        &self.notes
    }

    pub fn note(&self, id: NoteId) -> Option<&Note<'a>> {
        let idx = usize::try_from(id.get_u32() - 1).ok()?;
        self.notes.get(idx)
    }

    /// Walk every nested frame, note, and caption; collect required faces.
    pub fn admit(&self) -> Result<CheckedSheet<'_, 'a>, Error> {
        let count = self.notes.len();
        let mut faces = FaceRequirements::default();
        if count > 0 {
            faces.need_text(Cut::Roman);
        }
        admit_frames(&self.frames, count, 0, 0, &mut faces)?;
        for note in &self.notes {
            match note {
                Note::Dest { .. } => {
                    faces.need_text(Cut::Roman);
                }
                Note::Blocks(frames) => {
                    admit_frames(frames, count, 0, 0, &mut faces)?;
                }
            }
        }
        Ok(CheckedSheet { sheet: self, faces })
    }
}

/// Borrowed admitted sheet. Mutation is impossible while layout holds this.
/// `'s` is the borrow of the authoring sheet; `'a` is the sheet's content lifetime.
pub struct CheckedSheet<'s, 'a> {
    sheet: &'s Sheet<'a>,
    faces: FaceRequirements,
}

impl<'s, 'a> CheckedSheet<'s, 'a> {
    pub fn sheet(&self) -> &'s Sheet<'a> {
        self.sheet
    }

    pub fn width(&self) -> Measure {
        self.sheet.width
    }

    pub fn frames(&self) -> &'s [Frame<'a>] {
        &self.sheet.frames
    }

    pub fn notes(&self) -> &'s [Note<'a>] {
        &self.sheet.notes
    }

    pub fn note(&self, id: NoteId) -> Option<&'s Note<'a>> {
        self.sheet.note(id)
    }

    pub fn face_requirements(&self) -> FaceRequirements {
        self.faces.clone()
    }
}

fn admit_note_id(id: NoteId, count: usize, faces: &mut FaceRequirements) -> Result<(), Error> {
    let number = id.get_u32();
    let idx = usize::try_from(number.saturating_sub(1)).unwrap_or(usize::MAX);
    if idx >= count {
        return Err(Error::InvalidNote { number, count });
    }
    faces.need_text(Cut::Roman);
    Ok(())
}

fn admit_spans(
    spans: &[Span<'_>],
    count: usize,
    faces: &mut FaceRequirements,
) -> Result<(), Error> {
    for span in spans {
        match span {
            Span::Type { cut, text: _ } => faces.need_text(*cut),
            Span::Math(_) => {}
            Span::Note(id) => admit_note_id(*id, count, faces)?,
            Span::Strike(spans) => admit_spans(spans, count, faces)?,
        }
    }
    Ok(())
}

fn admit_frames(
    frames: &[Frame<'_>],
    count: usize,
    list_depth: u8,
    quote_depth: u8,
    faces: &mut FaceRequirements,
) -> Result<(), Error> {
    for frame in frames {
        admit_frame(frame, count, list_depth, quote_depth, faces)?;
    }
    Ok(())
}

fn admit_frame(
    frame: &Frame<'_>,
    count: usize,
    list_depth: u8,
    quote_depth: u8,
    faces: &mut FaceRequirements,
) -> Result<(), Error> {
    match frame {
        Frame::Source { location, frame } => {
            admit_frame(frame, count, list_depth, quote_depth, faces)
                .map_err(|e| location.annotate(e))
        }
        Frame::Text(block) => {
            faces.need_text(Cut::Roman);
            admit_spans(&block.spans, count, faces)
        }
        Frame::Head(_) => {
            faces.need_text(Cut::Bold);
            faces.need_text(Cut::Roman);
            Ok(())
        }
        Frame::Mark(mark) => {
            faces.need_display(mark.cut);
            Ok(())
        }
        Frame::Cols(cols) => {
            faces.need_text(Cut::Roman);
            match &cols.body {
                ColBody::Two { rows, .. } => {
                    for row in rows {
                        for cell in row {
                            admit_spans(cell, count, faces)?;
                        }
                    }
                }
                ColBody::Three { rows, .. } => {
                    for row in rows {
                        for cell in row {
                            admit_spans(cell, count, faces)?;
                        }
                    }
                }
            }
            Ok(())
        }
        Frame::List(list) => {
            if list_depth >= NEST_CAP {
                return Err(Error::Nesting);
            }
            faces.need_text(list.cut);
            for item in &list.items {
                match &item.body {
                    ItemBody::Blank => {}
                    ItemBody::Frames(nested) => {
                        admit_frames(nested.as_slice(), count, list_depth + 1, quote_depth, faces)?;
                    }
                }
            }
            Ok(())
        }
        Frame::Quote(quote) => {
            if quote_depth >= NEST_CAP {
                return Err(Error::Nesting);
            }
            admit_frames(&quote.frames, count, list_depth, quote_depth + 1, faces)
        }
        Frame::Code(_) => {
            faces.need_text(Cut::Mono);
            Ok(())
        }
        Frame::Figure(fig) => {
            if let Some(n) = fig.note() {
                admit_note_id(NoteId::from_index(n), count, faces)?;
            }
            Ok(())
        }
        Frame::Math(_) => Ok(()),
        Frame::Rule(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::DisplayCut;
    use crate::frame::blocks::{Code, Cols, List, ListFit, ListItem, Marker, Quote, Rule};
    use crate::frame::image::{Figure, Math};
    use crate::frame::inline::{Head, Mark, MarkAlign, TextBlock, Thickness, Tracking};
    use crate::leading::GridSkip;
    use crate::size::{DisplaySize, TextSize};
    use tm20::PRINTABLE_DOTS;

    fn nid(n: u32) -> NoteId {
        NoteId::from_index(NonZeroU32::new(n).unwrap())
    }

    fn plain(text: &str) -> Frame<'static> {
        Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            text.to_owned(),
        ))
    }

    fn text_with_note(id: NoteId) -> Frame<'static> {
        Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "see"), Span::note(id)],
        })
    }

    fn nest_lists(depth: u8, leaf: Frame<'static>) -> Frame<'static> {
        let mut frame = leaf;
        for _ in 0..depth {
            frame = Frame::List(List {
                size: TextSize::Pt11,
                cut: Cut::Roman,
                marker: Marker::Dash,
                fit: ListFit::Tight,
                items: vec![ListItem::new(vec![frame])],
            });
        }
        frame
    }

    fn nest_quotes(depth: u8, leaf: Frame<'static>) -> Frame<'static> {
        let mut frame = leaf;
        for _ in 0..depth {
            frame = Frame::Quote(Quote {
                frames: vec![frame],
            });
        }
        frame
    }

    #[test]
    fn measure_zero_and_over_tape_reject() {
        assert!(Measure::new(0).is_none());
        assert!(Measure::new(PRINTABLE_DOTS + 1).is_none());
        assert_eq!(Measure::new(1).unwrap().get(), 1);
        assert_eq!(Measure::TAPE.get(), PRINTABLE_DOTS);
    }

    #[test]
    fn add_note_is_complete_and_read_only() {
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
        assert_eq!(id.get_u32(), 1);
        assert_eq!(sheet.note_count(), 1);
        assert!(matches!(sheet.note(id), Some(Note::Dest { .. })));
        assert_eq!(sheet.notes().len(), 1);
    }

    #[test]
    fn builder_unknown_double_defined_and_unresolved_reject() {
        let mut b = SheetBuilder::new(Measure::TAPE);
        assert!(matches!(
            b.define_note(nid(1), Note::dest("x")),
            Err(Error::InvalidNote {
                number: 1,
                count: 0
            })
        ));

        let id = b.reserve_note().unwrap();
        b.define_note(id, Note::dest("a")).unwrap();
        assert!(matches!(
            b.define_note(id, Note::dest("b")),
            Err(Error::InvalidNote {
                number: 1,
                count: 1
            })
        ));

        let mut c = SheetBuilder::new(Measure::TAPE);
        let _ = c.reserve_note().unwrap();
        assert!(matches!(
            c.finish(),
            Err(Error::InvalidNote {
                number: 1,
                count: 1
            })
        ));
    }

    #[test]
    fn builder_forward_and_cyclic_ids_finish() {
        let mut b = SheetBuilder::new(Measure::TAPE);
        let one = b.reserve_note().unwrap();
        let two = b.reserve_note().unwrap();
        b.push_frame(text_with_note(one));
        b.define_note(one, Note::Blocks(vec![text_with_note(two)]))
            .unwrap();
        b.define_note(two, Note::Blocks(vec![text_with_note(one)]))
            .unwrap();
        let sheet = b.finish().unwrap();
        assert_eq!(sheet.note_count(), 2);
        assert!(sheet.admit().is_ok());
    }

    #[test]
    fn out_of_range_note_rejects_at_admit() {
        let sheet = Sheet::tape(vec![text_with_note(nid(1))]);
        assert!(matches!(
            sheet.admit(),
            Err(Error::InvalidNote {
                number: 1,
                count: 0
            })
        ));
    }

    #[test]
    fn three_deep_lists_pass_four_deep_reject() {
        let body3 = Sheet::tape(vec![nest_lists(3, plain("x"))]);
        assert!(body3.admit().is_ok());
        let body4 = Sheet::tape(vec![nest_lists(4, plain("x"))]);
        assert!(matches!(body4.admit(), Err(Error::Nesting)));

        let mut note3 = Sheet::tape(vec![]);
        let id = note3
            .add_note(Note::Blocks(vec![nest_lists(3, plain("n"))]))
            .unwrap();
        note3.frames.push(text_with_note(id));
        assert!(note3.admit().is_ok());

        let mut note4 = Sheet::tape(vec![]);
        let id = note4
            .add_note(Note::Blocks(vec![nest_lists(4, plain("n"))]))
            .unwrap();
        note4.frames.push(text_with_note(id));
        assert!(matches!(note4.admit(), Err(Error::Nesting)));
    }

    #[test]
    fn three_deep_quotes_pass_four_deep_reject() {
        let body3 = Sheet::tape(vec![nest_quotes(3, plain("q"))]);
        assert!(body3.admit().is_ok());
        let body4 = Sheet::tape(vec![nest_quotes(4, plain("q"))]);
        assert!(matches!(body4.admit(), Err(Error::Nesting)));
    }

    #[test]
    fn note_depth_starts_at_zero() {
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet
            .add_note(Note::Blocks(vec![nest_lists(3, plain("in note"))]))
            .unwrap();
        sheet.frames.push(nest_lists(3, text_with_note(id)));
        assert!(sheet.admit().is_ok());
    }

    #[test]
    fn figure_caption_note_is_checked() {
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet.add_note(Note::dest("caption")).unwrap();
        let fig = Figure::from_bits(1, 1, &[true]).unwrap().noted(id.get());
        sheet.frames.push(Frame::Figure(fig));
        assert!(sheet.admit().is_ok());

        let dangling = Figure::from_bits(1, 1, &[true])
            .unwrap()
            .noted(NonZeroU32::new(9).unwrap());
        let bad = Sheet::tape(vec![Frame::Figure(dangling)]);
        assert!(matches!(
            bad.admit(),
            Err(Error::InvalidNote {
                number: 9,
                count: 0
            })
        ));
    }

    #[test]
    fn admit_walks_every_frame_variant() {
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
        sheet.frames = vec![
            Frame::Text(TextBlock {
                size: TextSize::Pt11,
                spans: vec![Span::new(Cut::Roman, "a"), Span::note(id)],
            }),
            Frame::Head(Head {
                size: TextSize::Pt11,
                text: "H".into(),
            }),
            Frame::Mark(Mark {
                cut: DisplayCut::Roman,
                size: DisplaySize::Pt18,
                text: "M".into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }),
            Frame::Cols(Cols::two(
                TextSize::Pt11,
                GridSkip::ONE,
                [crate::frame::ColAlign::Start, crate::frame::ColAlign::End],
                vec![[vec![Span::note(id)], vec![Span::new(Cut::Roman, "x")]]],
            )),
            Frame::List(List {
                size: TextSize::Pt11,
                cut: Cut::Italic,
                marker: Marker::Dash,
                fit: ListFit::Tight,
                items: vec![ListItem::new(vec![]), ListItem::new(vec![plain("item")])],
            }),
            Frame::Quote(Quote { frames: vec![] }),
            Frame::Code(Code::new(TextSize::Pt11, "x")),
            Frame::Figure(Figure::from_bits(1, 1, &[true]).unwrap().noted(id.get())),
            Frame::Math(Math::from_bits(1, 1, &[true], 1).unwrap()),
            Frame::Rule(Rule::tape(Thickness::Two)),
        ];
        let checked = sheet.admit().expect("every variant is visited");
        let req = checked.face_requirements();
        assert!(req.text[Cut::Roman as usize]);
        assert!(req.text[Cut::Bold as usize]);
        assert!(req.text[Cut::Italic as usize]);
        assert!(req.text[Cut::Mono as usize]);
        assert!(req.display[DisplayCut::Roman as usize]);
        assert!(!req.display[DisplayCut::Light as usize]);
    }

    #[test]
    fn unused_light_is_not_required() {
        let sheet = Sheet::tape(vec![plain("hello")]);
        let req = sheet.admit().unwrap().face_requirements();
        assert!(req.text[Cut::Roman as usize]);
        assert!(!req.text[Cut::Mono as usize]);
        assert!(!req.display[DisplayCut::Light as usize]);
        assert!(!req.display[DisplayCut::Roman as usize]);
    }

    #[test]
    fn light_mark_requires_light() {
        let sheet = Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Light,
            size: DisplaySize::Pt14,
            text: "soft".into(),
            align: MarkAlign::Center,
            tracking: Tracking(0),
        })]);
        let req = sheet.admit().unwrap().face_requirements();
        assert!(req.display[DisplayCut::Light as usize]);
        assert!(!req.display[DisplayCut::Roman as usize]);
    }
}
