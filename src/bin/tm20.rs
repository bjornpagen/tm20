use std::env;
use std::io;
use std::process::ExitCode;
use std::time::Instant;

use tm20::identify::{encode_process_id, parse_process_id, query_info, InfoRequest};
use tm20::status::{encode_recover, encode_request, parse_status, StatusRequest};
use tm20::usb::WAIT_TIMEOUT;
use tm20::{
    catalog, ean13_page, encode, find_case, hello, qr_page, ruler, text_page, Transport, Usb,
};

const PROCESS_ID: [u8; 4] = *b"tm20";

fn usage() {
    eprintln!(
        "tm20 [--serial S] [--dry] [--wait] list | debug | hello | text <str> | ruler | status | recover | id | qr <data> | ean13 <digits> | test [id|all]\n  --wait sends GS ( H after the job and blocks until the printer replies\n  TM20_TRACE=1 logs USB timings"
    );
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    serial: Option<String>,
    dry: bool,
    wait: bool,
    args: Vec<String>,
}

fn parse_opts() -> io::Result<Opts> {
    let mut serial = None;
    let mut dry = false;
    let mut wait = false;
    let mut args = Vec::new();
    let mut raw = env::args().skip(1);
    while let Some(a) = raw.next() {
        match a.as_str() {
            "--serial" => {
                serial = Some(raw.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--serial needs a value")
                })?);
            }
            "--dry" => dry = true,
            "--wait" => wait = true,
            _ => {
                args.push(a);
                args.extend(raw);
                break;
            }
        }
    }
    Ok(Opts {
        serial,
        dry,
        wait,
        args,
    })
}

fn run() -> tm20::Result<()> {
    let opts = parse_opts()?;
    let mut cmd = opts.args.into_iter();
    match cmd.next().as_deref() {
        Some("list") => {
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
        Some("hello") => emit(&opts.serial, opts.dry, opts.wait, &encode(&hello())?),
        Some("text") => {
            let text = cmd.next().unwrap_or_default();
            emit(
                &opts.serial,
                opts.dry,
                opts.wait,
                &encode(&text_page(&text))?,
            )
        }
        Some("ruler") => emit(&opts.serial, opts.dry, opts.wait, &encode(&ruler())?),
        Some("status") => status(opts.serial.as_deref()),
        Some("debug") => debug(opts.serial.as_deref()),
        Some("id") => identify(opts.serial.as_deref()),
        Some("recover") => emit(&opts.serial, opts.dry, opts.wait, &encode_recover()),
        Some("qr") => {
            let data = cmd
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "qr needs data"))?;
            emit(&opts.serial, opts.dry, opts.wait, &encode(&qr_page(&data))?)
        }
        Some("ean13") => {
            let digits = cmd
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ean13 needs digits"))?;
            emit(
                &opts.serial,
                opts.dry,
                opts.wait,
                &encode(&ean13_page(&digits))?,
            )
        }
        Some("test") => test_cmd(cmd.next(), &opts.serial, opts.dry, opts.wait),
        _ => {
            usage();
            Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown command").into())
        }
    }
}

fn test_cmd(
    id: Option<String>,
    serial: &Option<String>,
    dry: bool,
    wait: bool,
) -> tm20::Result<()> {
    match id.as_deref() {
        None => {
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
        Some("all") => {
            if dry {
                for case in catalog().iter().filter(|c| c.in_all) {
                    emit(serial, true, wait, &encode(&case.doc())?)?;
                }
                return Ok(());
            }
            let mut usb = Usb::open(serial.as_deref())?;
            for case in catalog().iter().filter(|c| c.in_all) {
                eprintln!("printing {} ({})", case.id, case.expect);
                usb.write(&encode(&case.doc())?)?;
                if wait {
                    wait_done(&mut usb)?;
                }
            }
            Ok(())
        }
        Some(id) => {
            let case = find_case(id).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("unknown test {id}"))
            })?;
            eprintln!("{}: {}", case.id, case.expect);
            emit(serial, dry, wait, &encode(&case.doc())?)
        }
    }
}

fn debug(serial: Option<&str>) -> tm20::Result<()> {
    let t0 = Instant::now();
    let usb = Usb::open(serial)?;
    println!(
        "open {}ms  bulk OUT {:#04x} IN {}",
        t0.elapsed().as_millis(),
        usb.bulk_out(),
        usb.bulk_in()
            .map(|a| format!("{a:#04x}"))
            .unwrap_or_else(|| "none".into())
    );
    let p = usb.port_status()?;
    println!(
        "GET_PORT_STATUS {:#04x}  not_error={} selected={} paper_empty={}",
        p.byte, p.not_error, p.selected, p.paper_empty
    );
    Ok(())
}

fn identify(serial: Option<&str>) -> tm20::Result<()> {
    let mut usb = Usb::open(serial)?;
    for req in [
        InfoRequest::ModelId,
        InfoRequest::TypeId,
        InfoRequest::VersionId,
        InfoRequest::Firmware,
        InfoRequest::Manufacturer,
        InfoRequest::Name,
        InfoRequest::Serial,
        InfoRequest::Fonts,
    ] {
        let data = query_info(&mut usb, req)?;
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

fn status(serial: Option<&str>) -> tm20::Result<()> {
    let mut usb = Usb::open(serial)?;
    let requests = [
        StatusRequest::Printer,
        StatusRequest::OfflineCause,
        StatusRequest::ErrorCause,
        StatusRequest::RollPaper,
    ];
    for req in requests {
        usb.write(&encode_request(req))?;
        let mut buf = [0u8; 1];
        let n = usb.read(&mut buf)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "no status byte").into());
        }
        println!("{req:?}: {:#04x} {:?}", buf[0], parse_status(req, buf[0])?);
    }
    Ok(())
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

fn wait_done(usb: &mut Usb) -> tm20::Result<()> {
    usb.write(&encode_process_id(PROCESS_ID))?;
    let mut buf = [0u8; 64];
    let n = usb.read_timeout(&mut buf, WAIT_TIMEOUT)?;
    if n < 7 {
        return Err(tm20::IdentifyError::Unexpected {
            got: buf[..n].to_vec(),
        }
        .into());
    }
    let id = parse_process_id(&buf[..7])?;
    eprintln!("done {}", String::from_utf8_lossy(&id));
    Ok(())
}

fn emit(serial: &Option<String>, dry: bool, wait: bool, bytes: &[u8]) -> tm20::Result<()> {
    if dry {
        dump_hex(bytes);
        if wait {
            dump_hex(&encode_process_id(PROCESS_ID));
        }
        return Ok(());
    }
    let mut usb = Usb::open(serial.as_deref())?;
    usb.write(bytes)?;
    if wait {
        wait_done(&mut usb)?;
    }
    Ok(())
}
