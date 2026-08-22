use std::error::Error;
use std::fmt;

use crate::connector::ExternalKey;
use crate::transport::TransportError;

pub(crate) fn digest(namespace: &str, value: impl AsRef<[u8]>) -> ExternalKey {
    digest_parts(namespace, &[value.as_ref()])
}

pub(crate) fn digest_parts(namespace: &str, values: &[&[u8]]) -> ExternalKey {
    let mut hash = blake3::Hasher::new();
    hash.update(namespace.as_bytes());
    for value in values {
        hash.update(&(value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    ExternalKey(*hash.finalize().as_bytes())
}

pub(crate) fn provider_error(provider: &'static str, error: TransportError) -> ProviderError {
    ProviderError::Transport { provider, error }
}

#[derive(Debug)]
pub enum ProviderError {
    Transport {
        provider: &'static str,
        error: TransportError,
    },
    Cursor {
        provider: &'static str,
        reason: &'static str,
    },
    Protocol {
        provider: &'static str,
        reason: String,
    },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { provider, error } => write!(f, "{provider}: {error}"),
            Self::Cursor { provider, reason } => {
                write!(f, "{provider}: invalid cursor: {reason}")
            }
            Self::Protocol { provider, reason } => {
                write!(f, "{provider}: invalid provider response: {reason}")
            }
        }
    }
}

impl Error for ProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport { error, .. } => Some(error),
            Self::Cursor { .. } | Self::Protocol { .. } => None,
        }
    }
}
