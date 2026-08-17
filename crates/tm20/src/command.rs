//! [`Command`] is the language. Sticky style is written explicitly; encode
//! has no hidden printer state.

use crate::barcode::Barcode;
use crate::graphics::Graphics;
use crate::status::StatusRequest;
use crate::symbol::{DataMatrix, Gs1DataBar, MaxiCode, Pdf417, Qr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Underline {
    Off,
    Single,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineSpacing {
    Dots(u8),
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodePage {
    Pc437,
    Other(u8),
}

impl CodePage {
    pub fn byte(self) -> u8 {
        match self {
            CodePage::Pc437 => 0,
            CodePage::Other(n) => n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashDrawerPin {
    Pin2,
    Pin5,
}

/// Per-job print speed (`GS ( K` fn=50). Volatile; `Init` restores NV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintSpeed {
    /// Use the NV customized value (`a = 6`).
    Default,
    /// 1 = slow, 13 = fast.
    Level(u8),
}

impl PrintSpeed {
    pub fn level(n: u8) -> Option<Self> {
        (1..=13).contains(&n).then_some(Self::Level(n))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Cancel,
    /// Autocutter. TM-T20III is a partial cut (one point at left uncut).
    Cut,
    /// `n` line feeds (`0x0A`), matching morningprint `hello`.
    Feed {
        lines: u8,
    },
    /// `ESC J n` — print buffer and feed `n` dots.
    FeedDots {
        dots: u8,
    },
    /// `ESC SP n` — right-side character spacing in dots.
    CharSpacing {
        dots: u8,
    },
    /// `ESC $` — absolute print position from the left of the print area.
    AbsolutePosition {
        dots: u16,
    },
    /// `ESC \` — relative print position.
    RelativePosition {
        dots: i16,
    },
    /// `HT` — next horizontal tab.
    HorizontalTab,
    /// `ESC D` … `NUL`. Empty clears all tabs.
    SetTabs(Vec<u8>),
    /// `GS L` — left margin in dots.
    LeftMargin {
        dots: u16,
    },
    /// `GS W` — print area width in dots.
    PrintAreaWidth {
        dots: u16,
    },
    /// `GS ( K` fn=50 — per-job print speed.
    PrintSpeed(PrintSpeed),
    LineSpacing(LineSpacing),
    Align(Align),
    Font(Font),
    Bold(bool),
    Underline(Underline),
    DoubleStrike(bool),
    Invert(bool),
    UpsideDown(bool),
    /// `ESC V` — 90° clockwise in standard mode.
    Rotate90(bool),
    Size {
        width: u8,
        height: u8,
    },
    Smoothing(bool),
    CodePage(CodePage),
    CharacterSet(u8),
    Text(String),
    Raw(Vec<u8>),
    MotionUnits {
        x: u8,
        y: u8,
    },
    CashDrawer(CashDrawerPin),
    Barcode(Barcode),
    Qr(Qr),
    Pdf417(Pdf417),
    Gs1DataBar(Gs1DataBar),
    MaxiCode(MaxiCode),
    DataMatrix(DataMatrix),
    Graphics(Graphics),
    StatusRequest(StatusRequest),
}
