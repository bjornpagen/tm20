//! Bounded tm20 paper projections.
//!
//! Untrusted source strings are parsed into [`PaperText`] once. A digest is
//! packed against a dot budget and overflow is represented explicitly; there
//! is no unbounded continuation mode.

use std::error::Error;
use std::fmt;

use tm20::Command;
use tm20_set::{FaceTable, Measure};

use crate::policy::Privacy;

pub const MAX_TAPE_DOTS: u32 = 800;
pub const MAX_DIGEST_ITEMS: usize = 7;
const BODY_LINE_DOTS: u32 = 37;
const MASTHEAD_DOTS: u32 = 102;
const HEAD_DOTS: u32 = 37;
const RULE_DOTS: u32 = 18;
const FOOTER_DOTS: u32 = 37;
const CHARS_PER_LINE: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperText(String);

impl PaperText {
    pub fn parse(input: impl AsRef<str>) -> Result<Self, PaperError> {
        let input = input.as_ref();
        if input
            .chars()
            .any(|ch| ch == '\0' || (ch.is_control() && !ch.is_whitespace()))
        {
            return Err(PaperError::ControlCharacter);
        }
        let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            return Err(PaperError::EmptyText);
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCopy {
    Subject(PaperText),
    SubjectAndExcerpt {
        subject: PaperText,
        excerpt: PaperText,
    },
}

impl SourceCopy {
    pub fn metadata(subject: impl AsRef<str>) -> Result<Self, PaperError> {
        Ok(Self::Subject(PaperText::parse(subject)?))
    }

    pub fn excerpt(subject: impl AsRef<str>, excerpt: impl AsRef<str>) -> Result<Self, PaperError> {
        Ok(Self::SubjectAndExcerpt {
            subject: PaperText::parse(subject)?,
            excerpt: PaperText::parse(excerpt)?,
        })
    }

    pub fn project(self, privacy: Privacy) -> Result<ProjectedCopy, PaperError> {
        let text = match (privacy, self) {
            (
                Privacy::MetadataOnly,
                Self::Subject(subject) | Self::SubjectAndExcerpt { subject, .. },
            ) => subject,
            (
                Privacy::RedactedExcerpt | Privacy::FullExcerpt,
                Self::SubjectAndExcerpt { excerpt, .. },
            ) => excerpt,
            (Privacy::RedactedExcerpt | Privacy::FullExcerpt, Self::Subject(_)) => {
                return Err(PaperError::MissingExcerpt(privacy));
            }
        };
        Ok(ProjectedCopy { privacy, text })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedCopy {
    privacy: Privacy,
    text: PaperText,
}

impl ProjectedCopy {
    #[must_use]
    pub const fn privacy(&self) -> Privacy {
        self.privacy
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    People,
    Work,
    Mail,
    Network,
}

impl Section {
    const ALL: [Self; 4] = [Self::People, Self::Work, Self::Mail, Self::Network];

    const fn label(self) -> &'static str {
        match self {
            Self::People => "PEOPLE",
            Self::Work => "WORK",
            Self::Mail => "MAIL",
            Self::Network => "NETWORK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestItem {
    section: Section,
    source: PaperText,
    sender: PaperText,
    age: PaperText,
    summary: ProjectedCopy,
    updates: u32,
}

impl DigestItem {
    pub fn parse(
        section: Section,
        source: impl AsRef<str>,
        sender: impl AsRef<str>,
        age: impl AsRef<str>,
        summary: ProjectedCopy,
        updates: u32,
    ) -> Result<Self, PaperError> {
        Ok(Self {
            section,
            source: PaperText::parse(source)?,
            sender: PaperText::parse(sender)?,
            age: PaperText::parse(age)?,
            summary,
            updates,
        })
    }

    fn estimated_dots(&self) -> u32 {
        BODY_LINE_DOTS
            + wrapped_lines(self.summary.as_str()) * BODY_LINE_DOTS
            + u32::from(self.updates > 1) * BODY_LINE_DOTS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    title: PaperText,
    id: PaperText,
    items: Vec<DigestItem>,
    total_items: usize,
}

impl Digest {
    pub fn parse(
        title: impl AsRef<str>,
        id: impl AsRef<str>,
        items: Vec<DigestItem>,
        total_items: usize,
    ) -> Result<Self, PaperError> {
        if total_items < items.len() {
            return Err(PaperError::TotalBelowItems {
                total: total_items,
                items: items.len(),
            });
        }
        Ok(Self {
            title: PaperText::parse(title)?,
            id: PaperText::parse(id)?,
            items,
            total_items,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupt {
    title: PaperText,
    source: PaperText,
    sender: PaperText,
    age: PaperText,
    summary: ProjectedCopy,
    id: PaperText,
}

impl Interrupt {
    pub fn parse(
        title: impl AsRef<str>,
        source: impl AsRef<str>,
        sender: impl AsRef<str>,
        age: impl AsRef<str>,
        summary: ProjectedCopy,
        id: impl AsRef<str>,
    ) -> Result<Self, PaperError> {
        Ok(Self {
            title: PaperText::parse(title)?,
            source: PaperText::parse(source)?,
            sender: PaperText::parse(sender)?,
            age: PaperText::parse(age)?,
            summary,
            id: PaperText::parse(id)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEdition {
    pub markdown: String,
    pub estimated_height_dots: u32,
    pub included: usize,
    pub omitted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TapeBudget {
    used: u32,
}

impl TapeBudget {
    const fn new(base: u32) -> Self {
        Self { used: base }
    }

    fn admits(&self, dots: u32) -> bool {
        self.used.saturating_add(dots) <= MAX_TAPE_DOTS
    }

    fn charge(&mut self, dots: u32) {
        self.used += dots;
    }
}

#[must_use]
pub fn render_digest(digest: &Digest) -> RenderedEdition {
    let mut budget = TapeBudget::new(MASTHEAD_DOTS + RULE_DOTS + FOOTER_DOTS);
    let mut markdown = format!("# {}\n\n---\n", escape_markdown(digest.title.as_str()));
    let mut included = 0;
    let limit = digest.items.len().min(MAX_DIGEST_ITEMS);

    for section in Section::ALL {
        let candidates: Vec<&DigestItem> = digest
            .items
            .iter()
            .take(limit)
            .filter(|item| item.section == section)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let mut section_written = false;
        for item in candidates {
            let section_cost = if section_written { 0 } else { HEAD_DOTS };
            let cost = section_cost + item.estimated_dots();
            if !budget.admits(cost) {
                continue;
            }
            if !section_written {
                markdown.push_str("\n## ");
                markdown.push_str(section.label());
                markdown.push('\n');
                budget.charge(HEAD_DOTS);
                section_written = true;
            }
            markdown.push_str("\n**");
            markdown.push_str(&escape_markdown(item.source.as_str()));
            markdown.push_str(" · ");
            markdown.push_str(&escape_markdown(item.sender.as_str()));
            markdown.push_str(" · ");
            markdown.push_str(&escape_markdown(item.age.as_str()));
            markdown.push_str("**\n\n");
            markdown.push_str(&escape_markdown(item.summary.as_str()));
            markdown.push('\n');
            if item.updates > 1 {
                markdown.push_str("\n`");
                markdown.push_str(&item.updates.to_string());
                markdown.push_str(" updates`\n");
            }
            budget.charge(item.estimated_dots());
            included += 1;
        }
    }

    let omitted = digest.total_items.saturating_sub(included);
    markdown.push_str("\n---\n\n`");
    markdown.push_str(&escape_markdown(digest.id.as_str()));
    markdown.push('`');
    if omitted > 0 {
        markdown.push_str(" · ");
        markdown.push_str(&omitted.to_string());
        markdown.push_str(" more online");
    }
    markdown.push('\n');

    RenderedEdition {
        markdown,
        estimated_height_dots: budget.used,
        included,
        omitted,
    }
}

pub fn render_interrupt(interrupt: &Interrupt) -> Result<RenderedEdition, PaperError> {
    let summary_dots = wrapped_lines(interrupt.summary.as_str()) * BODY_LINE_DOTS;
    let estimated_height_dots =
        MASTHEAD_DOTS + RULE_DOTS + BODY_LINE_DOTS + summary_dots + FOOTER_DOTS;
    if estimated_height_dots > MAX_TAPE_DOTS {
        return Err(PaperError::TooTall {
            height: estimated_height_dots,
            limit: MAX_TAPE_DOTS,
        });
    }
    let markdown = format!(
        "# {}\n\n---\n\n**{} · {} · {}**\n\n{}\n\n---\n\n`{}`\n",
        escape_markdown(interrupt.title.as_str()),
        escape_markdown(interrupt.source.as_str()),
        escape_markdown(interrupt.sender.as_str()),
        escape_markdown(interrupt.age.as_str()),
        escape_markdown(interrupt.summary.as_str()),
        escape_markdown(interrupt.id.as_str()),
    );
    Ok(RenderedEdition {
        markdown,
        estimated_height_dots,
        included: 1,
        omitted: 0,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledEdition {
    pub document: tm20::Document,
    pub bytes: Vec<u8>,
    pub height_dots: u32,
}

pub fn compile(
    rendered: &RenderedEdition,
    faces: &FaceTable,
) -> Result<CompiledEdition, PaperError> {
    if rendered.estimated_height_dots > MAX_TAPE_DOTS {
        return Err(PaperError::TooTall {
            height: rendered.estimated_height_dots,
            limit: MAX_TAPE_DOTS,
        });
    }
    let sheet = tm20_md::sheet(&rendered.markdown, Measure::TAPE, |_| {
        Err(tm20_md::Error::Image)
    })
    .map_err(PaperError::Markdown)?;
    let document = tm20_set::lower(&sheet, faces).map_err(PaperError::Typeset)?;
    let height_dots = graphics_height(&document);
    if height_dots > MAX_TAPE_DOTS {
        return Err(PaperError::TooTall {
            height: height_dots,
            limit: MAX_TAPE_DOTS,
        });
    }
    let bytes = tm20::encode(&document).map_err(PaperError::Encode)?;
    Ok(CompiledEdition {
        document,
        bytes,
        height_dots,
    })
}

fn graphics_height(document: &tm20::Document) -> u32 {
    document
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Graphics(graphics) => Some(u32::from(graphics.height_dots)),
            _ => None,
        })
        .sum()
}

fn wrapped_lines(text: &str) -> u32 {
    let len = text.chars().count().max(1);
    u32::try_from(len.div_ceil(CHARS_PER_LINE)).unwrap_or(u32::MAX)
}

fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '#' | '|' | '!'
        ) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[derive(Debug)]
pub enum PaperError {
    EmptyText,
    ControlCharacter,
    MissingExcerpt(Privacy),
    TotalBelowItems { total: usize, items: usize },
    TooTall { height: u32, limit: u32 },
    Markdown(tm20_md::Error),
    Typeset(tm20_set::Error),
    Encode(tm20::EncodeError),
}

impl fmt::Display for PaperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => f.write_str("paper text is empty"),
            Self::ControlCharacter => f.write_str("paper text contains a control character"),
            Self::MissingExcerpt(privacy) => {
                write!(f, "{privacy:?} requires an excerpt-bearing source value")
            }
            Self::TotalBelowItems { total, items } => {
                write!(
                    f,
                    "digest total {total} is below its {items} supplied items"
                )
            }
            Self::TooTall { height, limit } => {
                write!(f, "edition is {height} dots tall; limit is {limit}")
            }
            Self::Markdown(error) => write!(f, "{error}"),
            Self::Typeset(error) => write!(f, "{error}"),
            Self::Encode(error) => write!(f, "{error}"),
        }
    }
}

impl Error for PaperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Markdown(error) => Some(error),
            Self::Typeset(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::EmptyText
            | Self::ControlCharacter
            | Self::MissingExcerpt(_)
            | Self::TotalBelowItems { .. }
            | Self::TooTall { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(section: Section, n: usize) -> DigestItem {
        let copy = SourceCopy::excerpt(
            "Notification",
            "A bounded summary that fits the narrow tape and contains no source markup.",
        )
        .expect("source copy")
        .project(Privacy::RedactedExcerpt)
        .expect("projected copy");
        DigestItem::parse(section, "Gmail", format!("sender {n}"), "2m", copy, 1).expect("item")
    }

    #[test]
    fn a_digest_is_bounded_and_names_its_overflow() {
        let items = (0..12).map(|n| item(Section::Mail, n)).collect::<Vec<_>>();
        let digest = Digest::parse("Now · 12:30", "D-0042", items, 12).expect("digest");
        let rendered = render_digest(&digest);
        assert!(rendered.estimated_height_dots <= MAX_TAPE_DOTS);
        assert!(rendered.included <= MAX_DIGEST_ITEMS);
        assert_eq!(rendered.included + rendered.omitted, 12);
        assert!(rendered.markdown.contains("more online"));
    }

    #[test]
    fn source_markup_is_data_not_markdown() {
        let copy = SourceCopy::excerpt("Private", "<div>do not parse me</div>")
            .expect("source copy")
            .project(Privacy::RedactedExcerpt)
            .expect("projected copy");
        let interrupt = Interrupt::parse("Now", "<script>", "**Mallory**", "now", copy, "I-1")
            .expect("interrupt");
        let rendered = render_interrupt(&interrupt).expect("render");
        tm20_md::sheet(&rendered.markdown, Measure::TAPE, |_| {
            Err(tm20_md::Error::Image)
        })
        .expect("escaped markdown lowers");
        assert!(rendered.markdown.contains("\\<script\\>"));
    }

    #[test]
    fn total_cannot_lie_below_the_supplied_projection() {
        assert!(matches!(
            Digest::parse("Now", "D-1", vec![item(Section::Mail, 1)], 0),
            Err(PaperError::TotalBelowItems { .. })
        ));
    }

    #[test]
    fn privacy_is_a_projection_not_a_render_time_flag() {
        let metadata = SourceCopy::excerpt("Subject only", "private body")
            .expect("source")
            .project(Privacy::MetadataOnly)
            .expect("metadata");
        assert_eq!(metadata.as_str(), "Subject only");
        assert!(matches!(
            SourceCopy::metadata("Subject only")
                .expect("source")
                .project(Privacy::RedactedExcerpt),
            Err(PaperError::MissingExcerpt(Privacy::RedactedExcerpt))
        ));
    }
}
