//! Closed sizes. No `f32` point constructor.

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eleven_pt_is_31_dots() {
        assert_eq!(TextSize::Pt11.body_dots(), 31);
    }
}
