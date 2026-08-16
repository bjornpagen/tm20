//! Real-time status: `DLE EOT` request and named-flag parse.

use crate::error::StatusError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusRequest {
    Printer,
    OfflineCause,
    ErrorCause,
    RollPaper,
}

impl StatusRequest {
    pub fn params(self) -> (u8, Option<u8>) {
        match self {
            StatusRequest::Printer => (1, None),
            StatusRequest::OfflineCause => (2, None),
            StatusRequest::ErrorCause => (3, None),
            StatusRequest::RollPaper => (4, None),
        }
    }
}

pub fn encode_request(request: StatusRequest) -> Vec<u8> {
    let (n, a) = request.params();
    let mut cmd = vec![0x10, 0x04, n];
    if let Some(a) = a {
        cmd.push(a);
    }
    cmd
}

/// `DLE ENQ 2` — recover from a recoverable error and clear buffers.
pub fn encode_recover() -> [u8; 3] {
    [0x10, 0x05, 2]
}

fn bit(byte: u8, n: u8) -> bool {
    (byte >> n) & 1 == 1
}

fn pattern_ok(byte: u8) -> bool {
    !bit(byte, 0) && bit(byte, 1) && bit(byte, 4) && !bit(byte, 7)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrinterStatus {
    pub drawer_kick_pin3_low: bool,
    pub online: bool,
    pub waiting_for_online_recovery: bool,
    pub paper_feed_button_pressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineCause {
    pub cover_closed: bool,
    pub paper_fed_by_button: bool,
    pub printing_stopped_paper_end: bool,
    pub error_occurred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCause {
    pub recoverable: bool,
    pub autocutter: bool,
    pub unrecoverable: bool,
    pub auto_recoverable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollPaper {
    pub near_end_adequate: bool,
    pub paper_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Printer(PrinterStatus),
    OfflineCause(OfflineCause),
    ErrorCause(ErrorCause),
    RollPaper(RollPaper),
}

pub fn parse_status(request: StatusRequest, byte: u8) -> Result<Status, StatusError> {
    if !pattern_ok(byte) {
        return Err(StatusError::BadPattern { byte });
    }
    Ok(match request {
        StatusRequest::Printer => Status::Printer(PrinterStatus {
            drawer_kick_pin3_low: !bit(byte, 2),
            online: !bit(byte, 3),
            waiting_for_online_recovery: bit(byte, 5),
            paper_feed_button_pressed: bit(byte, 6),
        }),
        StatusRequest::OfflineCause => Status::OfflineCause(OfflineCause {
            cover_closed: !bit(byte, 2),
            paper_fed_by_button: bit(byte, 3),
            printing_stopped_paper_end: bit(byte, 5),
            error_occurred: bit(byte, 6),
        }),
        StatusRequest::ErrorCause => Status::ErrorCause(ErrorCause {
            recoverable: bit(byte, 2),
            autocutter: bit(byte, 3),
            unrecoverable: bit(byte, 5),
            auto_recoverable: bit(byte, 6),
        }),
        StatusRequest::RollPaper => Status::RollPaper(RollPaper {
            near_end_adequate: !bit(byte, 2) && !bit(byte, 3),
            paper_present: !bit(byte, 5) && !bit(byte, 6),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_bytes() {
        assert_eq!(encode_request(StatusRequest::Printer), vec![0x10, 0x04, 1]);
        assert_eq!(
            encode_request(StatusRequest::RollPaper),
            vec![0x10, 0x04, 4]
        );
    }

    #[test]
    fn parse_printer() {
        let Status::Printer(s) = parse_status(StatusRequest::Printer, 0b0001_1010).unwrap() else {
            panic!("wrong variant");
        };
        assert!(s.drawer_kick_pin3_low);
        assert!(!s.online);
        assert!(!s.waiting_for_online_recovery);
        assert!(!s.paper_feed_button_pressed);
    }

    #[test]
    fn parse_offline() {
        let Status::OfflineCause(s) =
            parse_status(StatusRequest::OfflineCause, 0b0101_1110).unwrap()
        else {
            panic!("wrong variant");
        };
        assert!(!s.cover_closed);
        assert!(s.paper_fed_by_button);
        assert!(!s.printing_stopped_paper_end);
        assert!(s.error_occurred);
    }

    #[test]
    fn parse_error_cause() {
        let Status::ErrorCause(s) = parse_status(StatusRequest::ErrorCause, 0b0001_1010).unwrap()
        else {
            panic!("wrong variant");
        };
        assert!(!s.recoverable);
        assert!(s.autocutter);
        assert!(!s.unrecoverable);
        assert!(!s.auto_recoverable);
    }

    #[test]
    fn parse_roll_paper() {
        let Status::RollPaper(s) = parse_status(StatusRequest::RollPaper, 0b0001_0010).unwrap()
        else {
            panic!("wrong variant");
        };
        assert!(s.near_end_adequate);
        assert!(s.paper_present);
    }

    #[test]
    fn bad_pattern() {
        assert!(parse_status(StatusRequest::Printer, 0).is_err());
    }
}
