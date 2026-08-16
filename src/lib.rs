//! ESC/POS for the Epson TM-T20III.
//!
//! The public contract is a [`Document`] of [`Command`] values, [`encode`] to
//! bytes, and a [`Transport`] that writes them. USB (`04b8:0e28`) is the first
//! sink. There is no printer builder.

pub mod barcode;
pub mod command;
pub mod cp437;
pub mod document;
pub mod encode;
pub mod error;
pub mod graphics;
pub mod host;
pub mod identify;
pub mod memory;
pub mod net;
pub mod selftest;
pub mod serial;
pub mod status;
pub mod symbol;
pub mod transport;
pub mod typeface;
pub mod usb;

pub use barcode::{Barcode, BarcodeFont, BarcodeKind, BarcodeOptions, Code128Set, HriPosition};
pub use command::{Align, CashDrawerPin, CodePage, Command, CutKind, Font, LineSpacing, Underline};
pub use document::Document;
pub use encode::encode;
pub use error::{EncodeError, Error, IdentifyError, Result, StatusError, TypefaceError, UsbError};
pub use graphics::{pack, Graphics, GraphicsScale};
pub use host::{ean13_page, hello, qr_page, rule, ruler, text_page};
pub use identify::{encode_info, encode_process_id, parse_process_id, query_info, InfoRequest};
pub use memory::Memory;
pub use net::Tcp;
pub use selftest::{catalog, find as find_case, Case as TestCase};
pub use serial::Serial;
pub use status::{parse_status, Status, StatusRequest};
pub use symbol::{
    DataMatrix, DataMatrixType, Gs1DataBar, Gs1DataBarType, Gs1DataBarWidth, MaxiCode,
    MaxiCodeMode, Pdf417, Pdf417Ecc, Pdf417Kind, Qr, QrEcc, QrModel,
};
pub use transport::Transport;
pub use typeface::{
    raster as raster_typeface, Align as TypeAlign, Dots, Face, Line, Pt, Run, Weight as TypeWeight,
};
pub use usb::{PortStatus, Usb, UsbDeviceInfo};

pub const VID: u16 = 0x04b8;
pub const PID: u16 = 0x0e28;
pub const COLS_A: u8 = 48;
pub const COLS_B: u8 = 64;
pub const ROW_DOTS_A: u8 = 24;
pub const ROW_DOTS_B: u8 = 17;
pub const PRINTABLE_DOTS: u16 = 576;
