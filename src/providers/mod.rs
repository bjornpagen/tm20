//! Provider protocol scaffolding.
//!
//! Each module models the provider's real identity, cursor, replay, and effect
//! coordinates over [`crate::transport::HttpTransport`]. Tests use
//! deterministic transcripts; no credentials or live network implementation
//! are present.

mod common;

pub mod gmail;
pub mod google_chat;
pub mod hacker_news;
pub mod imessage;
pub mod slack;

pub use common::ProviderError;
pub(crate) use common::{digest, digest_parts, provider_error};
