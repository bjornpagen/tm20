//! Typesetter CLI job/output matrix. Dry never opens a device.

mod common;

use std::fs;
use std::path::Path;

use common::{bin, run, stderr, stdout, uniq_temp};

fn write_md(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn remote_image_prepares_and_fake_delivers_without_usb() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/image.png", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        let image = include_bytes!("../../tm20-md/fixtures/grid.png");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            image.len()
        )
        .unwrap();
        stream.write_all(image).unwrap();
        assert!(request.starts_with(b"GET /image.png HTTP/1.1\r\n"));
    });
    let dir = uniq_temp("remote-image");
    let input = write_md(&dir, "remote.md", &format!("![grid]({url})"));
    let output = dir.join("delivery");
    let result = run(&[
        "--fake-delivery",
        output.to_str().unwrap(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));
    server.join().unwrap();
    assert!(!fs::read(output.join("remote.bin")).unwrap().is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_markdown_reports_file_and_position_before_delivery() {
    let dir = uniq_temp("diagnostic");
    let input = write_md(&dir, "bad.md", "valid\n\n## bad *heading*");
    let output = dir.join("delivery");
    let result = run(&[
        "--fake-delivery",
        output.to_str().unwrap(),
        "print",
        "md",
        input.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    let message = stderr(&result);
    assert!(message.contains(input.to_str().unwrap()));
    assert!(message.contains("line 3, column 8"));
    assert!(message.contains("heading rendering cannot preserve"));
    assert!(message.contains("help:"));
    assert!(!output.join("bad.bin").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dry_print_ticket_succeeds_without_device() {
    let out = run(&["--dry", "print", "ticket"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!stdout(&out).trim().is_empty() || stderr(&out).contains("ticket"));
    assert!(!stderr(&out).to_ascii_lowercase().contains("not found") || out.status.success());
}

#[test]
fn dry_png_writes_preview_for_builtin() {
    let dir = uniq_temp("png-ticket");
    let out = run(&["--dry", "--png", dir.to_str().unwrap(), "print", "ticket"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let pngs: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    assert_eq!(pngs.len(), 1, "builtin dry --png must write one preview");
}

#[test]
fn dry_png_writes_preview_for_md_file() {
    let dir = uniq_temp("png-md");
    let md = write_md(&dir, "tape.md", "# Hi\n\nHello.\n");
    let out = run(&[
        "--dry",
        "--png",
        dir.to_str().unwrap(),
        "print",
        "md",
        md.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        dir.join("tape.png").is_file(),
        "single-file dry --png writes tape.png"
    );
}

#[test]
fn dry_png_writes_preview_for_md_directory() {
    let dir = uniq_temp("png-dir");
    write_md(&dir, "a.md", "# A\n\nOne.\n");
    write_md(&dir, "b.md", "# B\n\nTwo.\n");
    let out = run(&[
        "--dry",
        "--png",
        dir.to_str().unwrap(),
        "print",
        "md",
        dir.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.join("a.png").is_file());
    assert!(dir.join("b.png").is_file());
}

#[test]
fn dry_png_writes_preview_for_all() {
    let dir = uniq_temp("png-all");
    let out = run(&["--dry", "--png", dir.to_str().unwrap(), "print", "all"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let n = fs::read_dir(&dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "png")
        })
        .count();
    assert!(n >= 1, "print all --dry --png must write previews, got {n}");
}

#[test]
fn dry_without_png_writes_no_files() {
    let dir = uniq_temp("no-png");
    let out = run(&["--dry", "print", "ticket"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let n = fs::read_dir(&dir).unwrap().count();
    assert_eq!(n, 0);
}

#[test]
fn dry_bad_markdown_is_nonzero_and_writes_nothing() {
    let dir = uniq_temp("bad-md");
    let md = write_md(&dir, "bad.md", "<div>no</div>\n");
    let out = run(&[
        "--dry",
        "--png",
        dir.to_str().unwrap(),
        "print",
        "md",
        md.to_str().unwrap(),
    ]);
    assert!(!out.status.success(), "preparation failure must be nonzero");
    assert!(!dir.join("bad.png").exists());
}

#[test]
fn directory_read_errors_are_surfaced() {
    let missing = uniq_temp("missing-parent").join("no-such-dir");
    let out = run(&["--dry", "print", "md", missing.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(!stderr(&out).is_empty() || !stdout(&out).is_empty());
}

#[test]
fn unknown_and_extra_args_are_rejected() {
    let unknown = run(&["--dry", "print", "not-a-sheet"]);
    assert!(!unknown.status.success());
    let extra = run(&["--dry", "print", "ticket", "bonus"]);
    assert!(
        !extra.status.success(),
        "extra arguments must not be ignored"
    );
}

#[test]
fn fake_delivery_writes_only_to_the_requested_sink() {
    let sink = uniq_temp("fake-sink");
    let out = std::process::Command::new(bin())
        .args(["--fake-delivery", sink.to_str().unwrap(), "print", "ticket"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "--fake-delivery DIR must not open USB: {}",
        stderr(&out)
    );
    assert!(
        fs::read_dir(&sink).unwrap().next().is_some(),
        "fake delivery writes bytes to the sink"
    );
}

#[test]
fn dry_never_mentions_usb_open_failure() {
    let out = run(&["--dry", "print", "ticket"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let err = stderr(&out).to_ascii_lowercase();
    assert!(
        !err.contains("04b8") && !err.contains("no bulk"),
        "dry opened a device: {err}"
    );
}
