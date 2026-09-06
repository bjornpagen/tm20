//! 2D symbols: validation and `GS ( k` bytes.
//!
//! Packet lengths are derived from their bodies. Symbol families live in
//! `symbol/*.rs`.

mod datamatrix;
mod gs1;
mod maxicode;
mod packet;
mod pdf417;
mod qr;

pub use datamatrix::{DataMatrix, DataMatrixType, encode_data_matrix};
pub use gs1::{Gs1DataBar, Gs1DataBarType, Gs1DataBarWidth, encode_gs1};
pub use maxicode::{MaxiCode, MaxiCodeMode, encode_maxi};
pub use pdf417::{Pdf417, Pdf417Ecc, Pdf417Kind, encode_pdf417};
pub use qr::{Qr, QrEcc, QrModel, encode_qr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Qr,
    Pdf417,
    Gs1DataBar,
    MaxiCode,
    DataMatrix,
}
