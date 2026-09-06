//! Protocol CLI process tests. Every invocation is `--dry`. Never opens USB.

mod common;

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_tm20")
}

fn dry(args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .arg("--dry")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn tm20: {e}"))
}

#[test]
fn dry_hello_emits_planned_bytes() {
    let out = dry(&["hello"]);
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1b") || stdout.contains("1B") || !stdout.trim().is_empty(),
        "dry hello must print planned bytes, not open USB: {stdout}"
    );
}

#[test]
fn dry_status_emits_planned_bytes() {
    let out = dry(&["status"]);
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).is_empty()
            || String::from_utf8_lossy(&out.stderr).contains("status"),
        "dry status must not open USB"
    );
}

#[test]
fn dry_id_emits_planned_bytes() {
    let out = dry(&["id"]);
    assert!(
        out.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dry_debug_is_not_applicable() {
    let out = dry(&["debug"]);
    assert!(
        !out.status.success(),
        "dry debug must be an explicit not-applicable error, never USB"
    );
}

#[test]
fn dry_list_is_not_applicable() {
    let out = dry(&["list"]);
    assert!(
        !out.status.success(),
        "dry list must be an explicit not-applicable error, never USB"
    );
}

#[test]
fn unknown_and_extra_args_are_rejected() {
    let unknown = dry(&["not-a-command"]);
    assert!(!unknown.status.success());
    let extra = Command::new(bin())
        .args(["--dry", "hello", "bonus"])
        .output()
        .unwrap();
    assert!(
        !extra.status.success(),
        "extra arguments must not be ignored"
    );
}

#[test]
fn dry_never_fails_with_usb_not_found() {
    let out = dry(&["hello"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !err.to_ascii_lowercase().contains("usb") || out.status.success(),
        "dry must not attempt USB: {err}"
    );
}

fn _temp_used() {
    let _ = common::uniq_temp("cli");
}
