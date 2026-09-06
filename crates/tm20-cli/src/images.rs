//! Image I/O stays at the CLI boundary; the Markdown library receives bytes.

use reqwest::blocking::{Client, ClientBuilder};
use reqwest::{Url, header::LOCATION};
use std::path::Path;

use tm20_md::Error;

#[path = "image_proxy.rs"]
mod proxy;

/// Network permission is independent of preview/delivery mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImagePolicy {
    #[default]
    LocalOnly,
    AllowRemote,
}

pub fn load(policy: ImagePolicy, base: &Path, destination: &str) -> Result<Vec<u8>, Error> {
    let destination = destination.trim();
    let remote = destination.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    });
    if !remote {
        return tm20_md::image_bytes(base, destination);
    }
    // Deny before client construction, proxy discovery, DNS, or sockets.
    if policy == ImagePolicy::LocalOnly {
        return Err(Error::RemoteImageDenied {
            destination: destination.to_owned(),
        });
    }
    fetch(destination, proxy::client).map_err(|reason| Error::Resource {
        destination: destination.to_owned(),
        reason,
    })
}

fn client_builder() -> ClientBuilder {
    Client::builder()
        .timeout(None)
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
}

fn network_error(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut reason = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        reason.push_str(": ");
        reason.push_str(&cause.to_string());
        source = cause.source();
    }
    reason
}

fn fetch(
    destination: &str,
    client: impl Fn(&Url) -> Result<Client, String>,
) -> Result<Vec<u8>, String> {
    let mut url = Url::parse(destination).map_err(|e| e.to_string())?;
    // Resolve proxy policy again on each hop, including scheme changes.
    for hop in 0..=10 {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(
                "image redirects must use HTTP(S); use a local path for local images".into(),
            );
        }
        let mut response = client(&url)?
            .get(url.clone())
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(network_error)?;
        if response.status().is_redirection() {
            if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                return Err(format!(
                    "unexpected image HTTP status {}",
                    response.status()
                ));
            }
            if hop == 10 {
                return Err("image exceeded 10 redirects; use its final HTTP(S) URL".into());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .ok_or("image redirect has no Location header")?
                .to_str()
                .map_err(|_| "image redirect has an invalid Location header")?;
            url = url
                .join(location)
                .map_err(|e| format!("invalid image redirect: {e}"))?;
            continue;
        }
        let mut bytes = Vec::new();
        response.copy_to(&mut bytes).map_err(network_error)?;
        return Ok(bytes);
    }
    unreachable!("redirect limit returns inside loop")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn load_remote(destination: &str) -> Result<Vec<u8>, String> {
        fetch(destination, |_| {
            client_builder()
                .no_proxy()
                .build()
                .map_err(|e| e.to_string())
        })
    }

    #[test]
    fn default_policy_denies_all_http_spellings() {
        for url in [
            "http://image.invalid/a.png",
            " HTTPS://image.invalid/a.png ",
        ] {
            assert!(matches!(
                load(ImagePolicy::default(), Path::new("."), url),
                Err(Error::RemoteImageDenied { .. })
            ));
        }
    }

    #[test]
    fn local_images_need_no_network_permission() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert_eq!(
            load(ImagePolicy::default(), &base, "pig.png").unwrap(),
            include_bytes!("pig.png")
        );
    }

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
        let bytes = include_bytes!("pig.png").to_vec();
        let (url, server) = server(vec![
            ("302 Found\r\nLocation: /image.png?version=2", Vec::new()),
            ("200 OK", bytes.clone()),
        ]);
        assert_eq!(
            load_remote(&format!("{url}/start?version=1")).unwrap(),
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
        assert!(load_remote(&url).is_err());
        server.join().unwrap();
    }

    #[test]
    fn redirect_cannot_read_a_local_file() {
        let (url, server) = server(vec![(
            "302 Found\r\nLocation: file:///etc/hosts",
            Vec::new(),
        )]);
        assert!(load_remote(&url).is_err());
        server.join().unwrap();
    }

    #[test]
    fn redirect_loop_is_bounded() {
        let (url, server) = server(vec![("302 Found\r\nLocation: /again", Vec::new()); 11]);
        assert!(load_remote(&url).unwrap_err().contains("10 redirects"));
        assert_eq!(server.join().unwrap().len(), 11);
    }

    #[test]
    fn redirect_without_location_is_an_error() {
        let (url, server) = server(vec![("302 Found", Vec::new())]);
        assert!(load_remote(&url).unwrap_err().contains("no Location"));
        server.join().unwrap();
    }
}
