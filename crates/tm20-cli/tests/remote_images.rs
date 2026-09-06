//! Network tests are loopback-only; proxy settings belong to child processes.

mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{bin, stderr, uniq_temp};

fn command(proxy: &str) -> Command {
    let mut cmd = Command::new(bin());
    for key in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
        "REQUEST_METHOD",
    ] {
        cmd.env_remove(key);
    }
    cmd.env("ALL_PROXY", proxy);
    cmd
}

fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn assert_uncontacted(listener: &TcpListener) {
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

fn serve(listener: TcpListener) -> std::thread::JoinHandle<String> {
    serve_status(listener, "200 OK")
}

fn serve_status(listener: TcpListener, status: &'static str) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("test server did not receive a request: {e}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        let image = include_bytes!("../src/pig.png");
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            image.len()
        )
        .unwrap();
        stream.write_all(image).unwrap();
        String::from_utf8(request).unwrap()
    })
}

#[test]
fn default_denies_before_network_or_delivery_in_every_mode() {
    let origin = listener();
    let proxy = listener();
    let dir = uniq_temp("remote-denied");
    let input = dir.join("remote.md");
    fs::write(
        &input,
        format!(
            "hello\n\n![grid](http://{}/image.png)",
            origin.local_addr().unwrap()
        ),
    )
    .unwrap();
    let sink = dir.join("output");
    for mode in [
        vec![],
        vec!["--dry"],
        vec!["--fake-delivery", sink.to_str().unwrap()],
    ] {
        let result = command(&format!("http://{}", proxy.local_addr().unwrap()))
            .args(mode)
            .args([
                "--png",
                sink.to_str().unwrap(),
                "print",
                "md",
                input.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        let error = stderr(&result);
        assert!(error.contains("image.remote-denied"), "{error}");
        assert!(error.contains("line 3, column 1"), "{error}");
        assert!(error.contains(input.to_str().unwrap()), "{error}");
        assert!(error.contains("--allow-remote-images"), "{error}");
        assert!(!sink.exists());
    }
    assert_uncontacted(&origin);
    assert_uncontacted(&proxy);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn explicit_permission_fetches_through_environment_proxy() {
    let dir = uniq_temp("remote-proxy");
    let input = dir.join("remote.md");
    fs::write(&input, "![grid](http://image.invalid/image.png?version=2)").unwrap();
    let sink = dir.join("output");
    for mode in [
        vec!["--dry"],
        vec!["--fake-delivery", sink.to_str().unwrap()],
    ] {
        let proxy = listener();
        let address = format!("http://{}", proxy.local_addr().unwrap());
        let server = serve(proxy);
        let result = command(&address)
            .args(mode)
            .args([
                "--allow-remote-images",
                "print",
                "md",
                input.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", stderr(&result));
        assert!(
            server
                .join()
                .unwrap()
                .starts_with("GET http://image.invalid/image.png?version=2 HTTP/1.1\r\n")
        );
    }
    assert!(!fs::read(sink.join("remote.bin")).unwrap().is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn broken_proxy_never_falls_back_to_reachable_origin() {
    let origin = listener();
    let dir = uniq_temp("broken-proxy");
    let input = dir.join("remote.md");
    fs::write(
        &input,
        format!("![grid](http://{}/image.png)", origin.local_addr().unwrap()),
    )
    .unwrap();
    let proxy = listener();
    let address = format!("http://{}", proxy.local_addr().unwrap());
    let server = serve_status(proxy, "502 Bad Gateway");
    for proxy in [address, "http://127.0.0.1:0".into(), "http://[".into()] {
        let result = command(&proxy)
            .args([
                "--dry",
                "--allow-remote-images",
                "print",
                "md",
                input.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_uncontacted(&origin);
    }
    server.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn no_proxy_bypass_is_respected() {
    let origin = listener();
    let address = origin.local_addr().unwrap();
    let server = serve(origin);
    let proxy = listener();
    let dir = uniq_temp("no-proxy");
    let input = dir.join("remote.md");
    fs::write(&input, format!("![grid](http://{address}/image.png)")).unwrap();
    let result = command(&format!("http://{}", proxy.local_addr().unwrap()))
        .env("NO_PROXY", "127.0.0.1")
        .args([
            "--dry",
            "--allow-remote-images",
            "print",
            "md",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(
        server
            .join()
            .unwrap()
            .starts_with("GET /image.png HTTP/1.1\r\n")
    );
    assert_uncontacted(&proxy);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn redirect_from_bypassed_origin_uses_proxy_for_next_host() {
    let origin = listener();
    let address = origin.local_addr().unwrap();
    let first = serve_status(
        origin,
        "302 Found\r\nLocation: http://image.invalid/final.png",
    );
    let proxy = listener();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let second = serve(proxy);
    let dir = uniq_temp("redirect-proxy");
    let input = dir.join("remote.md");
    fs::write(&input, format!("![grid](http://{address}/start.png)")).unwrap();
    let result = command(&proxy_url)
        .env("NO_PROXY", "127.0.0.1")
        .args([
            "--dry",
            "--allow-remote-images",
            "print",
            "md",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(
        first
            .join()
            .unwrap()
            .starts_with("GET /start.png HTTP/1.1\r\n")
    );
    assert!(
        second
            .join()
            .unwrap()
            .starts_with("GET http://image.invalid/final.png HTTP/1.1\r\n")
    );
    fs::remove_dir_all(dir).unwrap();
}
