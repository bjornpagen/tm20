//! Closed sizes. No `f32` point constructor.

use crate::leading::Leading;

/// Thermal resolution of the TM-T20III.
pub const DPI: f32 = 203.0;

fn body_dots(pt: f32) -> u16 {
    (pt * DPI / 72.0).round() as u16
}

/// Text optical sizes. Unrepresentable on a [`crate::DisplayFace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Pt8,
    Pt11,
}

impl TextSize {
    pub fn pt(self) -> f32 {
        match self {
            TextSize::Pt8 => 8.0,
            TextSize::Pt11 => 11.0,
        }
    }

    pub fn body_dots(self) -> u16 {
        body_dots(self.pt())
    }

    /// Plus2 slug. Thermal ink spreads; one point of extra lead is not enough.
    pub fn skip_dots(self) -> u16 {
        Leading::Plus2.skip_dots(self.body_dots())
    }
}

/// Display optical sizes. Unrepresentable on a [`crate::TextFace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySize {
    Pt14,
    Pt18,
    Pt24,
}

impl DisplaySize {
    pub fn pt(self) -> f32 {
        match self {
            DisplaySize::Pt14 => 14.0,
            DisplaySize::Pt18 => 18.0,
            DisplaySize::Pt24 => 24.0,
        }
    }

    pub fn body_dots(self) -> u16 {
        body_dots(self.pt())
    }

    /// Solid slug.
    pub fn skip_dots(self) -> u16 {
        Leading::Solid.skip_dots(self.body_dots())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eleven_pt_is_31_dots() {
        assert_eq!(TextSize::Pt11.body_dots(), 31);
    }

    #[test]
    fn eleven_pt_skip_is_plus2() {
        assert_eq!(TextSize::Pt11.skip_dots(), 37);
    }

    #[test]
    fn eight_pt_skip_is_plus2() {
        assert_eq!(TextSize::Pt8.body_dots(), 23);
        assert_eq!(TextSize::Pt8.skip_dots(), 29);
    }

    #[test]
    fn display_eighteen_is_solid() {
        assert_eq!(DisplaySize::Pt18.body_dots(), 51);
        assert_eq!(DisplaySize::Pt18.skip_dots(), 51);
    }
}
