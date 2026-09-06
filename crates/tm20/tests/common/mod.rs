//! Shared protocol integration helpers. Not a test binary.
//! Each integration binary uses a subset; unused items stay for the others.
#![allow(dead_code)]

use std::io::{self, ErrorKind};
use std::path::PathBuf;

use tm20::error::Result;
use tm20::transport::Transport;

/// Scripted transport: scheduled read chunks, including Interrupted and EOF.
pub struct Scripted {
    pub written: Vec<u8>,
    reads: Vec<io::Result<Vec<u8>>>,
    index: usize,
}

impl Scripted {
    pub fn new(reads: Vec<io::Result<Vec<u8>>>) -> Self {
        Self {
            written: Vec::new(),
            reads,
            index: 0,
        }
    }
}

impl Transport for Scripted {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.written.extend_from_slice(data);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.index >= self.reads.len() {
            return Ok(0);
        }
        let next = &self.reads[self.index];
        self.index += 1;
        match next {
            Ok(bytes) => {
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                Ok(n)
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                Err(io::Error::new(ErrorKind::Interrupted, "interrupted").into())
            }
            Err(e) => Err(io::Error::new(e.kind(), e.to_string()).into()),
        }
    }
}

/// Every ordered partition of `bytes` into nonempty contiguous chunks.
pub fn partitions(bytes: &[u8]) -> Vec<Vec<Vec<u8>>> {
    if bytes.is_empty() {
        return vec![vec![]];
    }
    let n = bytes.len();
    let mut out = Vec::new();
    let max = 1usize << (n.saturating_sub(1));
    for mask in 0..max {
        let mut chunks = Vec::new();
        let mut start = 0usize;
        for i in 0..n.saturating_sub(1) {
            if mask & (1 << i) != 0 {
                chunks.push(bytes[start..=i].to_vec());
                start = i + 1;
            }
        }
        chunks.push(bytes[start..].to_vec());
        out.push(chunks);
    }
    out
}

/// Independent GS ( k walker. Declared length must match the following body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsK {
    pub cn: u8,
    pub fn_: u8,
    pub body: Vec<u8>,
}

pub fn walk_gs_k(bytes: &[u8]) -> std::result::Result<Vec<GsK>, String> {
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < bytes.len() {
        if i + 5 > bytes.len() {
            return Err(format!(
                "truncated header at {i}, leftover {}",
                bytes.len() - i
            ));
        }
        if bytes[i] != 0x1d || bytes[i + 1] != b'(' || bytes[i + 2] != b'k' {
            return Err(format!(
                "expected GS ( k at {i}, leftover {:?}",
                &bytes[i..]
            ));
        }
        let pl = usize::from(bytes[i + 3]);
        let ph = usize::from(bytes[i + 4]);
        let len = pl + 256 * ph;
        let start = i + 5;
        let end = start.checked_add(len).ok_or("length overflow")?;
        if end > bytes.len() {
            return Err(format!("declared length {len} overruns buffer at {i}"));
        }
        if len < 2 {
            return Err(format!("GS ( k body shorter than cn fn at {i}"));
        }
        out.push(GsK {
            cn: bytes[start],
            fn_: bytes[start + 1],
            body: bytes[start + 2..end].to_vec(),
        });
        i = end;
    }
    Ok(out)
}

pub fn uniq_temp(prefix: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "tm20-{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    p
}
