//! Typed errors. Payloads are data; [`Display`] formats them.

use std::fmt;
use std::io;

use crate::barcode::BarcodeKind;

#[derive(Debug)]
pub enum Error {
    Encode(EncodeError),
    Usb(UsbError),
    Status(StatusError),
    Identify(IdentifyError),
    Io(io::Error),
    Serial(serialport::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Encode(e) => write!(f, "{e}"),
            Error::Usb(e) => write!(f, "{e}"),
            Error::Status(e) => write!(f, "{e}"),
            Error::Identify(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Serial(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<EncodeError> for Error {
    fn from(e: EncodeError) -> Self {
        Error::Encode(e)
    }
}

impl From<UsbError> for Error {
    fn from(e: UsbError) -> Self {
        Error::Usb(e)
    }
}

impl From<StatusError> for Error {
    fn from(e: StatusError) -> Self {
        Error::Status(e)
    }
}

impl From<IdentifyError> for Error {
    fn from(e: IdentifyError) -> Self {
        Error::Identify(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self {
        Error::Serial(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    Size { width: u8, height: u8 },
    BarcodeData { kind: BarcodeKind },
    BarcodeWidth(u8),
    BarcodeHeight(u8),
    BarcodeTooLong { len: usize },
    QrTooLong { len: usize },
    QrSize(u8),
    Pdf417Columns(u8),
    Pdf417Rows(u8),
    Pdf417Width(u8),
    Pdf417RowHeight(u8),
    Pdf417CorrectionRatio(u8),
    DataMatrixSize(u8),
    DataMatrixType { rows: u8, cols: u8 },
    Gs1DataEmpty,
    Gs2dTooLong { len: usize },
    GraphicsPackedLen { expected: usize, got: usize },
    GraphicsTooLong { len: usize },
    PrintSpeed(u8),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::Size { width, height } => {
                write!(f, "size width={width} height={height} not in 1..=8")
            }
            EncodeError::BarcodeData { kind } => write!(f, "invalid {kind:?} payload"),
            EncodeError::BarcodeWidth(n) => write!(f, "barcode width {n} not in 1..=6"),
            EncodeError::BarcodeHeight(n) => write!(f, "barcode height {n} is 0"),
            EncodeError::BarcodeTooLong { len } => {
                write!(f, "barcode payload length {len} exceeds 255")
            }
            EncodeError::QrTooLong { len } => write!(f, "QR data length {len} exceeds 7089"),
            EncodeError::QrSize(n) => write!(f, "QR module size {n} not in 0..=16"),
            EncodeError::Pdf417Columns(n) => write!(f, "PDF417 columns {n} not in 0..=30"),
            EncodeError::Pdf417Rows(n) => write!(f, "PDF417 rows {n} not in 0 or 3..=90"),
            EncodeError::Pdf417Width(n) => write!(f, "PDF417 width {n} not in 0 or 2..=8"),
            EncodeError::Pdf417RowHeight(n) => {
                write!(f, "PDF417 row height {n} not in 0 or 2..=8")
            }
            EncodeError::Pdf417CorrectionRatio(n) => {
                write!(f, "PDF417 correction ratio {n} not in 1..=40")
            }
            EncodeError::DataMatrixSize(n) => write!(f, "DataMatrix size {n} not in 2..=16"),
            EncodeError::DataMatrixType { rows, cols } => {
                write!(f, "DataMatrix type {rows}x{cols} is not a legal size")
            }
            EncodeError::Gs1DataEmpty => write!(f, "GS1 DataBar data is empty"),
            EncodeError::Gs2dTooLong { len } => {
                write!(f, "GS ( k payload length {len} exceeds 65535")
            }
            EncodeError::GraphicsPackedLen { expected, got } => {
                write!(f, "graphics packed length {got}, expected {expected}")
            }
            EncodeError::GraphicsTooLong { len } => {
                write!(f, "graphics payload length {len} exceeds 65535")
            }
            EncodeError::PrintSpeed(n) => write!(f, "print speed {n} not in 1..=13"),
        }
    }
}

impl std::error::Error for EncodeError {}

#[derive(Debug)]
pub enum UsbError {
    NotFound {
        vid: u16,
        pid: u16,
        serial: Option<String>,
    },
    NoBulkOut,
    Nusb(nusb::Error),
    Transfer(io::Error),
}

impl fmt::Display for UsbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbError::NotFound { vid, pid, serial } => match serial {
                Some(s) => write!(f, "USB {vid:04x}:{pid:04x} serial {s} not found"),
                None => write!(f, "USB {vid:04x}:{pid:04x} not found"),
            },
            UsbError::NoBulkOut => write!(f, "no bulk OUT endpoint"),
            UsbError::Nusb(e) => write!(f, "{e}"),
            UsbError::Transfer(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for UsbError {}

impl From<nusb::Error> for UsbError {
    fn from(e: nusb::Error) -> Self {
        UsbError::Nusb(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusError {
    BadPattern { byte: u8 },
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusError::BadPattern { byte } => {
                write!(f, "status byte {byte:#04x} is not 0xx1xx10")
            }
        }
    }
}

impl std::error::Error for StatusError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifyError {
    Unexpected { got: Vec<u8> },
}

impl fmt::Display for IdentifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentifyError::Unexpected { got } => {
                write!(f, "unexpected identity reply {got:?}")
            }
        }
    }
}

impl std::error::Error for IdentifyError {}
