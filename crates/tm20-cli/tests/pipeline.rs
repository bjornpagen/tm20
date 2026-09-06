//! Process-boundary contracts, using files, pipes and loopback only.
mod common;

use common::{bin, run, stderr, uniq_temp};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        Self(uniq_temp("pipeline"))
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stdin(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_and_file_encode_identically_and_stdout_is_only_escpos() {
    let tmp = Tmp::new();
    let file = tmp.0.join("tape.md");
    let input = "# Hello\n\nText with **bold**, *italic*, ~~strike~~, and `code`.\n";
    fs::write(&file, input).unwrap();
    let from_file = run(&["--output", "-", "print", "md", file.to_str().unwrap()]);
    let from_stdin = stdin(&["--output", "-", "print", "md", "-"], input.as_bytes());
    assert!(from_file.status.success(), "{}", stderr(&from_file));
    assert!(from_stdin.status.success(), "{}", stderr(&from_stdin));
    assert!(from_file.stdout.starts_with(&[0x1b, b'@']));
    assert_eq!(from_stdin.stdout, from_file.stdout);
    assert!(stderr(&from_stdin).contains("<stdin>"));
}

#[test]
fn failed_stdin_keeps_diagnostics_and_configured_failure_status() {
    let out = stdin(
        &[
            "--failure-exit-code",
            "125",
            "--output",
            "-",
            "print",
            "md",
            "-",
        ],
        b"valid\n\n## bad *heading*\n",
    );
    assert_eq!(out.status.code(), Some(125));
    assert!(out.stdout.is_empty());
    let message = stderr(&out);
    assert!(
        message.contains("<stdin>") && message.contains("line 3") && message.contains("help:"),
        "{message}"
    );
    let invalid_utf8 = stdin(&["--dry", "print", "md", "-"], &[0xff]);
    assert!(!invalid_utf8.status.success());
}

#[test]
fn stdin_resolves_images_against_explicit_base() {
    let tmp = Tmp::new();
    fs::write(tmp.0.join("pig.png"), include_bytes!("../src/pig.png")).unwrap();
    let out = stdin(
        &[
            "--dry",
            "--base-dir",
            tmp.0.to_str().unwrap(),
            "print",
            "md",
            "-",
        ],
        b"![pig](pig.png)\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(out.stdout.is_empty());
}

#[test]
fn batch_raw_output_is_sorted_and_matches_individual_jobs() {
    let tmp = Tmp::new();
    let input = tmp.0.join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("b.md"), "Second.").unwrap();
    fs::write(input.join("a.md"), "First.").unwrap();
    let sink = tmp.0.join("jobs");
    let raw = tmp.0.join("batch.bin");
    let out = run(&[
        "--fake-delivery",
        sink.to_str().unwrap(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = run(&[
        "--output",
        raw.to_str().unwrap(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(out.stdout.is_empty());
    assert_eq!(
        fs::read(raw).unwrap(),
        [
            fs::read(sink.join("a.bin")).unwrap(),
            fs::read(sink.join("b.bin")).unwrap()
        ]
        .concat()
    );
}

#[test]
fn failed_preparation_preserves_existing_output_and_never_connects() {
    let tmp = Tmp::new();
    let input = tmp.0.join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("a.md"), "Valid first job.").unwrap();
    fs::write(input.join("b.md"), "<div>invalid</div>").unwrap();
    let raw = tmp.0.join("out.bin");
    fs::write(&raw, b"keep").unwrap();
    let out = run(&[
        "--output",
        raw.to_str().unwrap(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert_eq!(fs::read(raw).unwrap(), b"keep");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let out = run(&[
        "--tcp",
        &listener.local_addr().unwrap().to_string(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

fn capture_tcp(binary: &str, args: &[&str]) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("expected loopback connection: {e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let out = Command::new(binary)
        .args(["--tcp", &addr.to_string()])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(out.stdout.is_empty());
    server.join().unwrap()
}

#[test]
fn tcp_delivery_sends_exact_prepared_bytes_once() {
    let raw = run(&["--output", "-", "print", "ticket"]);
    assert!(raw.status.success(), "{}", stderr(&raw));
    assert_eq!(capture_tcp(bin(), &["print", "ticket"]), raw.stdout);
}

#[test]
fn protocol_cli_uses_the_same_tcp_destination() {
    assert_eq!(
        capture_tcp(env!("CARGO_BIN_EXE_tm20"), &["hello"]),
        tm20::encode(&tm20::hello()).unwrap()
    );
}

#[test]
fn usage_control_endpoints_are_device_free() {
    for binary in [bin(), env!("CARGO_BIN_EXE_tm20")] {
        for arg in ["--help", "--version", "__usage_spec__"] {
            let out = Command::new(binary).arg(arg).output().unwrap();
            assert!(out.status.success(), "{}", stderr(&out));
            assert!(!out.stdout.is_empty());
        }
    }
    let out = run(&["font-licenses"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("SIL OPEN FONT LICENSE"));
}

#[test]
fn success_stays_zero_with_custom_failure_status() {
    let out = run(&["--failure-exit-code", "125", "--dry", "print", "ticket"]);
    assert_eq!(out.status.code(), Some(0));
    let out = Command::new(env!("CARGO_BIN_EXE_tm20"))
        .args(["--failure-exit-code", "125", "--dry", "debug"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(125));
}

#[test]
fn portable_is_default_and_macos_is_explicit() {
    let default = run(&["--output", "-", "print", "ticket"]);
    let portable = run(&["--fonts", "portable", "--output", "-", "print", "ticket"]);
    assert!(default.status.success() && portable.status.success());
    assert_eq!(default.stdout, portable.stdout);
    #[cfg(target_os = "macos")]
    {
        let macos = run(&["--fonts", "macos", "--output", "-", "print", "ticket"]);
        assert!(macos.status.success(), "{}", stderr(&macos));
        assert_ne!(macos.stdout, portable.stdout);
    }
}
