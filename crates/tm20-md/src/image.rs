//! Resolve a CommonMark image destination to bytes.

use std::path::{Path, PathBuf};

use crate::error::Error;

/// Read figure bytes for a CommonMark image destination.
///
/// `base` is the directory of the markdown file. Relative dests and `file:`
/// URLs are read from disk. Other schemes (HTTP, data, mailto) fail; a caller
/// that wants those supplies its own `load`.
pub fn image_bytes(base: &Path, dest: &str) -> Result<Vec<u8>, Error> {
    let path = image_path(base, dest)?;
    std::fs::read(path).map_err(|_| Error::Image)
}

fn image_path(base: &Path, dest: &str) -> Result<PathBuf, Error> {
    let dest = cut_query_frag(dest.trim());
    if dest.is_empty() {
        return Err(Error::Image);
    }
    match scheme(dest) {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("file") => file_path(base, rest),
        Some(_) => Err(Error::Image),
        None => local_path(base, dest),
    }
}

fn local_path(base: &Path, dest: &str) -> Result<PathBuf, Error> {
    let decoded = percent_decode(dest).ok_or(Error::Image)?;
    let path = Path::new(&decoded);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(base.join(path))
    }
}

fn file_path(base: &Path, rest: &str) -> Result<PathBuf, Error> {
    if let Some(after) = rest.strip_prefix("//") {
        let slash = after.find('/').ok_or(Error::Image)?;
        let host = authority_host(&after[..slash]);
        if !local_host(host) {
            return Err(Error::Image);
        }
        local_path(base, &after[slash..])
    } else {
        local_path(base, rest)
    }
}

fn authority_host(auth: &str) -> &str {
    let hostport = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    if let Some(rest) = hostport.strip_prefix('[') {
        return rest.split_once(']').map_or(hostport, |(h, _)| h);
    }
    hostport.split_once(':').map_or(hostport, |(h, _)| h)
}

fn local_host(host: &str) -> bool {
    host.is_empty() || host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1"
}

fn scheme(dest: &str) -> Option<(&str, &str)> {
    let colon = dest.find(':')?;
    let scheme = &dest[..colon];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some((scheme, &dest[colon + 1..]))
}

fn cut_query_frag(s: &str) -> &str {
    let s = s.split_once('#').map_or(s, |(h, _)| h);
    s.split_once('?').map_or(s, |(h, _)| h)
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let h = nibble(*bytes.get(i + 1)?)?;
            let l = nibble(*bytes.get(i + 2)?)?;
            out.push((h << 4) | l);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    fn is_png(bytes: &[u8]) -> bool {
        bytes.starts_with(&[0x89, b'P', b'N', b'G'])
    }

    #[test]
    fn relative_dest_reads_the_file() {
        assert!(is_png(&image_bytes(&fixtures(), "grid.png").unwrap()));
    }

    #[test]
    fn dot_relative_dest_reads_the_file() {
        assert!(is_png(&image_bytes(&fixtures(), "./grid.png").unwrap()));
    }

    #[test]
    fn percent_encoded_dest_reads_the_file() {
        assert!(is_png(&image_bytes(&fixtures(), "grid%2epng").unwrap()));
    }

    #[test]
    fn file_url_reads_the_file() {
        let path = fixtures().join("grid.png");
        let dest = format!("file://{}", path.display());
        assert!(is_png(&image_bytes(&fixtures(), &dest).unwrap()));
    }

    #[test]
    fn file_localhost_url_reads_the_file() {
        let path = fixtures().join("grid.png");
        let dest = format!("file://localhost{}", path.display());
        assert!(is_png(&image_bytes(&fixtures(), &dest).unwrap()));
    }

    #[test]
    fn http_dest_is_an_error() {
        assert!(matches!(
            image_bytes(&fixtures(), "https://example.com/grid.png"),
            Err(Error::Image)
        ));
    }

    #[test]
    fn empty_dest_is_an_error() {
        assert!(matches!(image_bytes(&fixtures(), ""), Err(Error::Image)));
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(matches!(
            image_bytes(&fixtures(), "no-such.png"),
            Err(Error::Image)
        ));
    }
}
