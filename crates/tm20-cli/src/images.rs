//! Image I/O stays at the CLI boundary; the Markdown library receives bytes.

use std::path::Path;
use std::process::Command;

use tm20_md::Error;

pub fn load(base: &Path, destination: &str) -> Result<Vec<u8>, Error> {
    let destination = destination.trim();
    let remote = destination.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    });
    if !remote {
        return tm20_md::image_bytes(base, destination);
    }
    // curl supplies the platform TLS stack and redirect handling. Disable its
    // per-user config so document loading doesn't inherit output/protocol flags.
    let output = Command::new("curl")
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--globoff",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--url",
            destination,
        ])
        .output()
        .map_err(|e| Error::Resource {
            destination: destination.to_owned(),
            reason: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(Error::Resource {
            destination: destination.to_owned(),
            reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn server(
        responses: Vec<(&'static str, Vec<u8>)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (headers, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                requests.push(first);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 {headers}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });
        (url, handle)
    }

    #[test]
    fn remote_images_follow_redirects_and_preserve_query() {
        let bytes = include_bytes!("../../tm20-md/fixtures/grid.png").to_vec();
        let (url, server) = server(vec![
            ("302 Found\r\nLocation: /image.png?version=2", Vec::new()),
            ("200 OK", bytes.clone()),
        ]);
        assert_eq!(
            load(Path::new("."), &format!("{url}/start?version=1")).unwrap(),
            bytes
        );
        assert_eq!(
            server.join().unwrap(),
            [
                "GET /start?version=1 HTTP/1.1\r\n",
                "GET /image.png?version=2 HTTP/1.1\r\n",
            ]
        );
    }

    #[test]
    fn http_error_is_not_image_data() {
        let (url, server) = server(vec![("404 Not Found", b"not an image".to_vec())]);
        assert!(load(Path::new("."), &url).is_err());
        server.join().unwrap();
    }

    #[test]
    fn redirect_cannot_read_a_local_file() {
        let (url, server) = server(vec![(
            "302 Found\r\nLocation: file:///etc/hosts",
            Vec::new(),
        )]);
        assert!(load(Path::new("."), &url).is_err());
        server.join().unwrap();
    }
}
