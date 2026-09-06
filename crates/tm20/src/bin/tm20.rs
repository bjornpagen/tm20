//! Protocol CLI. Dry never opens USB. `status`/`id` print planned bytes;
//! `debug`/`list` return an explicit not-applicable error.
//!
//! [`parse`] is pure; [`wait_done`] is the production completion
//! helper (ReplyReader + exact requested ID). Delivery writes through
//! [`ReplyReader`]; tests inject [`tm20::Memory`]. Dry never constructs USB.

use std::env;
use std::io;
use std::process::ExitCode;
use std::time::Instant;

use tm20::identify::{InfoRequest, encode_info, encode_process_id, query_info};
use tm20::status::{StatusRequest, encode_recover, encode_request, parse_status};
use tm20::{
    ReplyReader, Transport, Usb, catalog, ean13_page, encode, find_case, hello, qr_page, ruler,
    text_page,
};

const PROCESS_ID: [u8; 4] = *b"tm20";

const STATUS_REQUESTS: [StatusRequest; 4] = [
    StatusRequest::Printer,
    StatusRequest::OfflineCause,
    StatusRequest::ErrorCause,
    StatusRequest::RollPaper,
];

const INFO_REQUESTS: [InfoRequest; 8] = [
    InfoRequest::ModelId,
    InfoRequest::TypeId,
    InfoRequest::VersionId,
    InfoRequest::Firmware,
    InfoRequest::Manufacturer,
    InfoRequest::Name,
    InfoRequest::Serial,
    InfoRequest::Fonts,
];

fn usage() {
    eprintln!(
        "tm20 [--serial S] [--dry] [--wait] list | debug | hello | text <str> | ruler | status | recover | id | qr <data> | ean13 <digits> | test [id|all]\n  --wait sends GS ( H after the job and blocks until the printer replies\n  --dry prints planned bytes and never opens USB; debug and list cannot dry-run\n  TM20_TRACE=1 logs USB timings"
    );
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Dry {
        wait: bool,
    },
    Deliver {
        selector: Option<String>,
        wait: bool,
    },
}

impl Mode {
    fn is_dry(&self) -> bool {
        matches!(self, Self::Dry { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    List,
    Debug,
    Hello,
    Text(String),
    Ruler,
    Status,
    Recover,
    Id,
    Qr(String),
    Ean13(String),
    TestList,
    TestAll,
    TestOne(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parsed {
    mode: Mode,
    command: Command,
}

fn parse<I, S>(args: I) -> io::Result<Parsed>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut serial = None;
    let mut dry = false;
    let mut wait = false;
    let mut rest = Vec::new();
    let mut raw = args.into_iter().map(|s| s.as_ref().to_owned());
    while let Some(a) = raw.next() {
        match a.as_str() {
            "--serial" => {
                serial = Some(raw.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--serial needs a value")
                })?);
            }
            "--dry" => dry = true,
            "--wait" => wait = true,
            other if other.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option {other}"),
                ));
            }
            other => {
                rest.push(other.to_owned());
                rest.extend(raw);
                break;
            }
        }
    }

    let mode = if dry {
        Mode::Dry { wait }
    } else {
        Mode::Deliver {
            selector: serial,
            wait,
        }
    };

    let mut cmd = rest.into_iter();
    let command = match cmd.next().as_deref() {
        Some("list") => {
            reject_extra(cmd.next())?;
            Command::List
        }
        Some("debug") => {
            reject_extra(cmd.next())?;
            Command::Debug
        }
        Some("hello") => {
            reject_extra(cmd.next())?;
            Command::Hello
        }
        Some("text") => {
            let text = cmd.next().unwrap_or_default();
            reject_extra(cmd.next())?;
            Command::Text(text)
        }
        Some("ruler") => {
            reject_extra(cmd.next())?;
            Command::Ruler
        }
        Some("status") => {
            reject_extra(cmd.next())?;
            Command::Status
        }
        Some("recover") => {
            reject_extra(cmd.next())?;
            Command::Recover
        }
        Some("id") => {
            reject_extra(cmd.next())?;
            Command::Id
        }
        Some("qr") => {
            let data = cmd
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "qr needs data"))?;
            reject_extra(cmd.next())?;
            Command::Qr(data)
        }
        Some("ean13") => {
            let digits = cmd
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ean13 needs digits"))?;
            reject_extra(cmd.next())?;
            Command::Ean13(digits)
        }
        Some("test") => match cmd.next() {
            None => Command::TestList,
            Some(id) if id == "all" => {
                reject_extra(cmd.next())?;
                Command::TestAll
            }
            Some(id) => {
                reject_extra(cmd.next())?;
                Command::TestOne(id)
            }
        },
        Some(other) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown command {other}"),
            ));
        }
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown command",
            ));
        }
    };

    Ok(Parsed { mode, command })
}

fn reject_extra(extra: Option<String>) -> io::Result<()> {
    match extra {
        Some(a) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected argument {a}"),
        )),
        None => Ok(()),
    }
}

fn dry_not_applicable(cmd: &str) -> tm20::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("dry-run is not applicable to {cmd}"),
    )
    .into()
}

fn run(args: impl IntoIterator<Item = impl AsRef<str>>) -> tm20::Result<()> {
    let parsed = match parse(args) {
        Ok(p) => p,
        Err(e) => {
            if e.to_string().contains("unknown command") {
                usage();
            }
            return Err(e.into());
        }
    };
    match parsed.command {
        Command::List => {
            if parsed.mode.is_dry() {
                return Err(dry_not_applicable("list"));
            }
            list_usb()
        }
        Command::Debug => {
            if parsed.mode.is_dry() {
                return Err(dry_not_applicable("debug"));
            }
            let Mode::Deliver { selector, .. } = &parsed.mode else {
                unreachable!("debug is device-only");
            };
            debug(selector.as_deref())
        }
        Command::Status => {
            if parsed.mode.is_dry() {
                dry_status();
                return Ok(());
            }
            with_usb(&parsed.mode, status_on)
        }
        Command::Id => {
            if parsed.mode.is_dry() {
                dry_id();
                return Ok(());
            }
            with_usb(&parsed.mode, identify_on)
        }
        Command::Hello => emit_doc(&parsed.mode, &encode(&hello())?),
        Command::Text(text) => emit_doc(&parsed.mode, &encode(&text_page(&text))?),
        Command::Ruler => emit_doc(&parsed.mode, &encode(&ruler())?),
        Command::Recover => emit_doc(&parsed.mode, &encode_recover()),
        Command::Qr(data) => emit_doc(&parsed.mode, &encode(&qr_page(&data))?),
        Command::Ean13(digits) => emit_doc(&parsed.mode, &encode(&ean13_page(&digits))?),
        Command::TestList => test_list(),
        Command::TestAll => test_all(&parsed.mode),
        Command::TestOne(id) => test_one(&parsed.mode, &id),
    }
}

fn with_usb<F>(mode: &Mode, f: F) -> tm20::Result<()>
where
    F: FnOnce(&mut ReplyReader<Usb>) -> tm20::Result<()>,
{
    let Mode::Deliver { selector, .. } = mode else {
        return Err(dry_not_applicable("device"));
    };
    let mut reader = ReplyReader::new(Usb::open(selector.as_deref())?);
    f(&mut reader)
}

fn emit_doc(mode: &Mode, bytes: &[u8]) -> tm20::Result<()> {
    match mode {
        Mode::Dry { wait } => {
            dump_hex(bytes);
            if *wait {
                dump_hex(&encode_process_id(PROCESS_ID));
            }
            Ok(())
        }
        Mode::Deliver { selector, wait } => {
            let mut reader = ReplyReader::new(Usb::open(selector.as_deref())?);
            deliver(&mut reader, bytes, *wait)
        }
    }
}

fn deliver<T: Transport>(
    reader: &mut ReplyReader<T>,
    bytes: &[u8],
    wait: bool,
) -> tm20::Result<()> {
    reader.transport_mut().write(bytes)?;
    if wait {
        wait_done(reader)?;
    }
    Ok(())
}

/// Production completion: write GS ( H, then require the requested ID.
fn wait_done<T: Transport>(reader: &mut ReplyReader<T>) -> tm20::Result<[u8; 4]> {
    reader
        .transport_mut()
        .write(&encode_process_id(PROCESS_ID))?;
    let id = reader.read_process_completion(PROCESS_ID)?;
    eprintln!("done {}", String::from_utf8_lossy(&id));
    Ok(id)
}

fn dry_status() {
    for req in STATUS_REQUESTS {
        dump_hex(&encode_request(req));
    }
}

fn dry_id() {
    for req in INFO_REQUESTS {
        dump_hex(&encode_info(req));
    }
}

#[cfg(test)]
fn planned_status_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    for req in STATUS_REQUESTS {
        out.extend(encode_request(req));
    }
    out
}

#[cfg(test)]
fn planned_id_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    for req in INFO_REQUESTS {
        out.extend(encode_info(req));
    }
    out
}

fn list_usb() -> tm20::Result<()> {
    for d in tm20::usb::list()? {
        let mark = if d.is_tm20() { "*" } else { " " };
        println!(
            "{mark} {vid:04x}:{pid:04x}  {man} {prod}  serial={serial}  bus={bus} addr={addr}",
            vid = d.vid,
            pid = d.pid,
            man = d.manufacturer.as_deref().unwrap_or("-"),
            prod = d.product.as_deref().unwrap_or("-"),
            serial = d.serial.as_deref().unwrap_or("-"),
            bus = d.bus_id,
            addr = d.address,
        );
    }
    Ok(())
}

fn debug(serial: Option<&str>) -> tm20::Result<()> {
    let t0 = Instant::now();
    let usb = Usb::open(serial)?;
    println!(
        "open {}ms  bulk OUT {:#04x} IN {}",
        t0.elapsed().as_millis(),
        usb.bulk_out(),
        usb.bulk_in()
            .map_or_else(|| "none".into(), |a| format!("{a:#04x}"))
    );
    let p = usb.port_status()?;
    println!(
        "GET_PORT_STATUS {:#04x}  not_error={} selected={} paper_empty={}",
        p.byte, p.not_error, p.selected, p.paper_empty
    );
    Ok(())
}

fn identify_on<T: Transport>(reader: &mut ReplyReader<T>) -> tm20::Result<()> {
    for req in INFO_REQUESTS {
        let data = query_info(reader, req)?;
        match req {
            InfoRequest::ModelId | InfoRequest::TypeId | InfoRequest::VersionId => {
                println!("{req:?}: {:#04x}", data[0]);
            }
            _ => {
                println!("{req:?}: {}", String::from_utf8_lossy(&data));
            }
        }
    }
    Ok(())
}

fn status_on<T: Transport>(reader: &mut ReplyReader<T>) -> tm20::Result<()> {
    for req in STATUS_REQUESTS {
        reader.transport_mut().write(&encode_request(req))?;
        let buf = reader.read_exact_reply(1)?;
        println!("{req:?}: {:#04x} {:?}", buf[0], parse_status(req, buf[0])?);
    }
    Ok(())
}

fn test_list() -> tm20::Result<()> {
    println!("id          in-all  what to look for");
    for case in catalog() {
        println!(
            "{:<11} {:<6}  {} — {}",
            case.id,
            if case.in_all { "yes" } else { "no" },
            case.title,
            case.expect
        );
    }
    println!();
    println!("tm20 test <id> | tm20 test all");
    println!("status / identity are separate: tm20 status | tm20 id");
    Ok(())
}

fn test_all(mode: &Mode) -> tm20::Result<()> {
    let jobs: Vec<_> = catalog()
        .iter()
        .filter(|c| c.in_all)
        .map(|c| Ok((c.id, c.expect, encode(&c.doc())?)))
        .collect::<tm20::Result<Vec<_>>>()?;
    match mode {
        Mode::Dry { wait } => {
            for (_, _, bytes) in &jobs {
                dump_hex(bytes);
                if *wait {
                    dump_hex(&encode_process_id(PROCESS_ID));
                }
            }
            Ok(())
        }
        Mode::Deliver { selector, wait } => {
            let mut reader = ReplyReader::new(Usb::open(selector.as_deref())?);
            for (id, expect, bytes) in &jobs {
                eprintln!("printing {id} ({expect})");
                deliver(&mut reader, bytes, *wait)?;
            }
            Ok(())
        }
    }
}

fn test_one(mode: &Mode, id: &str) -> tm20::Result<()> {
    let case = find_case(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("unknown test {id}")))?;
    eprintln!("{}: {}", case.id, case.expect);
    emit_doc(mode, &encode(&case.doc())?)
}

fn dump_hex(bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && i % 16 == 0 {
            println!();
        }
        print!("{b:02x} ");
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tm20::{Error, FramingError, Memory};

    fn parse_ok(args: &[&str]) -> Parsed {
        parse(args).unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
    }

    fn parse_err(args: &[&str]) -> String {
        parse(args).unwrap_err().to_string()
    }

    #[test]
    fn leading_options_and_known_spellings() {
        let p = parse_ok(&["--serial", "S", "--dry", "--wait", "hello"]);
        assert_eq!(p.mode, Mode::Dry { wait: true });
        assert_eq!(p.command, Command::Hello);
        assert_eq!(parse_ok(&["text"]).command, Command::Text(String::new()));
        assert_eq!(
            parse_ok(&["text", "hi"]).command,
            Command::Text("hi".into())
        );
        assert_eq!(parse_ok(&["test"]).command, Command::TestList);
        assert_eq!(parse_ok(&["test", "all"]).command, Command::TestAll);
        assert_eq!(parse_ok(&["qr", "x"]).command, Command::Qr("x".into()));
    }

    #[test]
    fn unknown_options_and_extra_positionals_are_rejected() {
        assert!(parse_err(&["--foo", "hello"]).contains("unknown option --foo"));
        assert!(parse_err(&["hello", "x"]).contains("unexpected argument x"));
        assert!(parse_err(&["status", "x"]).contains("unexpected argument x"));
        assert!(parse_err(&["test", "all", "x"]).contains("unexpected argument x"));
        assert!(parse_err(&["text", "a", "b"]).contains("unexpected argument b"));
        assert!(parse_err(&["qr"]).contains("qr needs data"));
        assert!(parse_err(&[]).contains("unknown command"));
        assert!(parse_err(&["nope"]).contains("unknown command nope"));
        assert!(parse_err(&["--serial"]).contains("--serial needs a value"));
    }

    #[test]
    fn dry_status_and_id_are_the_planned_request_bytes() {
        assert_eq!(
            planned_status_bytes(),
            [
                encode_request(StatusRequest::Printer),
                encode_request(StatusRequest::OfflineCause),
                encode_request(StatusRequest::ErrorCause),
                encode_request(StatusRequest::RollPaper),
            ]
            .concat()
        );
        assert_eq!(
            planned_id_bytes(),
            INFO_REQUESTS
                .iter()
                .flat_map(|r| encode_info(*r))
                .collect::<Vec<_>>()
        );
        assert!(!planned_status_bytes().is_empty());
        assert!(!planned_id_bytes().is_empty());
    }

    #[test]
    fn dry_debug_and_list_are_not_applicable() {
        let err = run(["--dry", "debug"]).unwrap_err();
        assert!(
            err.to_string()
                .contains("dry-run is not applicable to debug")
        );
        let err = run(["--dry", "list"]).unwrap_err();
        assert!(
            err.to_string()
                .contains("dry-run is not applicable to list")
        );
    }

    #[test]
    fn dry_hello_does_not_touch_memory() {
        let before = Memory::new();
        assert!(before.written.is_empty());
        run(["--dry", "hello"]).unwrap();
        run(["--dry", "--wait", "hello"]).unwrap();
        run(["--dry", "status"]).unwrap();
        run(["--dry", "id"]).unwrap();
        run(["--dry", "test"]).unwrap();
    }

    #[test]
    fn wait_done_rejects_wrong_id_through_production_helper() {
        let reply = [0x37, 0x22, b'x', b'x', b'x', b'x', 0];
        let mut reader = ReplyReader::new(Memory::with_replies(reply.to_vec()));
        match wait_done(&mut reader) {
            Err(Error::Framing(FramingError::WrongId {
                requested,
                got: [b'x', b'x', b'x', b'x'],
            })) if requested == PROCESS_ID => {}
            other => panic!("expected WrongId, got {other:?}"),
        }
        assert_eq!(reader.transport().written, encode_process_id(PROCESS_ID));
    }

    #[test]
    fn wait_done_accepts_requested_id() {
        let reply = [0x37, 0x22, b't', b'm', b'2', b'0', 0];
        let mut reader = ReplyReader::new(Memory::with_replies(reply.to_vec()));
        assert_eq!(wait_done(&mut reader).unwrap(), PROCESS_ID);
    }

    #[test]
    fn deliver_on_memory_records_job_then_optional_wait() {
        let reply = [0x37, 0x22, b't', b'm', b'2', b'0', 0];
        let bytes = encode(&hello()).unwrap();
        let mut reader = ReplyReader::new(Memory::with_replies(reply.to_vec()));
        deliver(&mut reader, &bytes, true).unwrap();
        let written = &reader.transport().written;
        assert!(written.starts_with(&bytes));
        assert!(written.ends_with(&encode_process_id(PROCESS_ID)));
    }

    #[test]
    fn dry_test_all_prepares_without_opening() {
        run(["--dry", "test", "all"]).unwrap();
    }
}
