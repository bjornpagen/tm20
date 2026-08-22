//! Invented iMessage bridge protocol.
//!
//! The bridge service is intentionally separate. This module defines the
//! contract it must implement: monotonic event backfill, push envelopes as
//! hints, stable message/conversation IDs, explicit mutation variants, and
//! idempotent read receipts.

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, EffectVerb, ExternalKey, NormalizedObservation, ObservationKind,
    SourceClass, TypedConnector, TypedPullBatch,
};
use crate::providers::{ProviderError, digest, digest_parts, provider_error};
use crate::transport::{HttpRequest, HttpTransport};

const KEY: ConnectorKey = ConnectorKey::new("imessage-bridge");
const CAPABILITIES: &[Capability] = &[
    Capability::Reconcile,
    Capability::Effect(EffectVerb::SendReadReceipt),
];
const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IMessageCursor {
    pub bridge_id: String,
    pub sequence: u64,
}

impl CursorCodec for IMessageCursor {
    fn encode(&self) -> CursorToken {
        CursorToken::parse(serde_json::to_vec(self).expect("IMessageCursor is JSON-safe"))
            .expect("serialized cursor is nonempty")
    }

    fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
        serde_json::from_slice(token.as_bytes())
            .map_err(|_| ConnectorError::InvalidCursor("iMessage cursor is not valid JSON"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub bridge_id: String,
    pub protocol_version: u16,
    pub capabilities: BridgeCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeCapabilities {
    pub event_backfill: bool,
    pub push_events: bool,
    pub read_receipts: bool,
    pub attachments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeEventPage {
    pub events: Vec<BridgeEvent>,
    pub next_sequence: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeEvent {
    pub sequence: u64,
    pub event_id: String,
    pub occurred_at: String,
    pub kind: BridgeEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEventKind {
    MessageCreated { message: IMessage },
    MessageEdited {
        conversation_id: String,
        message_id: String,
        revision: u64,
        text: String,
    },
    MessageUnsent {
        conversation_id: String,
        message_id: String,
    },
    TapbackAdded {
        conversation_id: String,
        message_id: String,
        actor: String,
        reaction: Tapback,
    },
    TapbackRemoved {
        conversation_id: String,
        message_id: String,
        actor: String,
        reaction: Tapback,
    },
    AttachmentAvailable {
        conversation_id: String,
        message_id: String,
        attachment: Attachment,
    },
    ReadReceiptObserved {
        conversation_id: String,
        reader: String,
        through_message_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IMessage {
    pub id: String,
    pub conversation_id: String,
    pub sender: String,
    pub sent_at: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub media_type: String,
    pub byte_count: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tapback {
    Love,
    Like,
    Dislike,
    Laugh,
    Emphasize,
    Question,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IMessageEvent {
    bridge_id: Box<str>,
    event: BridgeEvent,
    occurred_at: i64,
}

impl From<IMessageEvent> for NormalizedObservation {
    fn from(value: IMessageEvent) -> Self {
        let (conversation, message, kind) = event_coordinates(&value.event.kind);
        let payload = serde_json::to_vec(&value.event.kind).expect("BridgeEventKind is JSON-safe");
        Self {
            account: digest("imessage-bridge", value.bridge_id.as_bytes()),
            object: digest_parts(
                "imessage-message",
                &[value.bridge_id.as_bytes(), message.as_bytes()],
            ),
            thread: digest_parts(
                "imessage-conversation",
                &[value.bridge_id.as_bytes(), conversation.as_bytes()],
            ),
            event: digest("imessage-event", value.event.event_id.as_bytes()),
            payload: digest("imessage-payload", payload),
            kind,
            occurred_at: value.occurred_at,
        }
    }
}

fn event_coordinates(kind: &BridgeEventKind) -> (&str, &str, ObservationKind) {
    match kind {
        BridgeEventKind::MessageCreated { message } => (
            &message.conversation_id,
            &message.id,
            ObservationKind::Created,
        ),
        BridgeEventKind::MessageEdited {
            conversation_id,
            message_id,
            ..
        }
        | BridgeEventKind::TapbackAdded {
            conversation_id,
            message_id,
            ..
        }
        | BridgeEventKind::TapbackRemoved {
            conversation_id,
            message_id,
            ..
        }
        | BridgeEventKind::AttachmentAvailable {
            conversation_id,
            message_id,
            ..
        } => (conversation_id, message_id, ObservationKind::Updated),
        BridgeEventKind::MessageUnsent {
            conversation_id,
            message_id,
        } => (conversation_id, message_id, ObservationKind::Deleted),
        BridgeEventKind::ReadReceiptObserved {
            conversation_id,
            through_message_id,
            ..
        } => (
            conversation_id,
            through_message_id,
            ObservationKind::ReadStateChanged,
        ),
    }
}

fn parse_millis(value: &str) -> Result<i64, ProviderError> {
    let time = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| ProviderError::Protocol {
        provider: "imessage",
        reason: format!("event time is not RFC3339: {error}"),
    })?;
    i64::try_from(time.unix_timestamp_nanos() / 1_000_000).map_err(|_| {
        ProviderError::Protocol {
            provider: "imessage",
            reason: "event time does not fit i64 milliseconds".into(),
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IMessageReadReceipt {
    conversation_id: Box<str>,
    through_message_id: Box<str>,
    idempotency: ExternalKey,
}

impl TryFrom<EffectRequest> for IMessageReadReceipt {
    type Error = ConnectorError;

    fn try_from(request: EffectRequest) -> Result<Self, Self::Error> {
        if request.verb != EffectVerb::SendReadReceipt {
            return Err(ConnectorError::EffectForAnotherProvider);
        }
        let locator = request
            .target
            .locator
            .as_str()
            .strip_prefix("imessage:conversation:")
            .ok_or(ConnectorError::EffectForAnotherProvider)?;
        let (conversation, message) = locator
            .split_once(":message:")
            .filter(|(conversation, message)| !conversation.is_empty() && !message.is_empty())
            .ok_or(ConnectorError::EffectForAnotherProvider)?;
        Ok(Self {
            conversation_id: conversation.into(),
            through_message_id: message.into(),
            idempotency: request.idempotency,
        })
    }
}

#[derive(Debug, Serialize)]
struct ReadReceiptRequest<'a> {
    through_message_id: &'a str,
    idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReceiptStatus {
    Applied,
    AlreadyApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReadReceiptResponse {
    receipt_id: String,
    status: ReceiptStatus,
    applied_at: String,
}

pub struct IMessageConnector<T> {
    http: T,
    pushed: VecDeque<BridgeEvent>,
    seen_events: HashSet<String>,
}

impl<T> IMessageConnector<T> {
    pub fn new(http: T) -> Self {
        Self {
            http,
            pushed: VecDeque::new(),
            seen_events: HashSet::new(),
        }
    }

    pub fn push_event(&mut self, event: BridgeEvent) {
        if self.seen_events.insert(event.event_id.clone()) {
            self.pushed.push_back(event);
        }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.http
    }
}

impl<T> TypedConnector for IMessageConnector<T>
where
    T: HttpTransport + 'static,
{
    type Cursor = IMessageCursor;
    type Event = IMessageEvent;
    type Effect = IMessageReadReceipt;
    type Error = ProviderError;

    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            key: KEY,
            label: "iMessage bridge",
            class: SourceClass::PersonalMessaging,
            capabilities: CAPABILITIES,
        }
    }

    fn pull(
        &mut self,
        cursor: Option<&Self::Cursor>,
    ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error> {
        let info: BridgeInfo = self
            .http
            .execute(HttpRequest::get("/bridge/v1/info"))
            .map_err(|error| provider_error("imessage", error))?
            .json()
            .map_err(|error| provider_error("imessage", error))?;
        if info.protocol_version != PROTOCOL_VERSION || !info.capabilities.event_backfill {
            return Err(ProviderError::Protocol {
                provider: "imessage",
                reason: "bridge protocol version or backfill capability is incompatible".into(),
            });
        }
        if cursor.is_some_and(|cursor| cursor.bridge_id != info.bridge_id) {
            return Err(ProviderError::Cursor {
                provider: "imessage",
                reason: "bridge identity changed",
            });
        }
        let start = cursor.map_or(0, |cursor| cursor.sequence);
        let mut after = start;
        let mut events = Vec::new();
        let mut seen = HashSet::new();
        while let Some(event) = self.pushed.pop_front() {
            if event.sequence > start && seen.insert(event.event_id.clone()) {
                events.push(event);
            }
        }
        loop {
            let page: BridgeEventPage = self
                .http
                .execute(
                    HttpRequest::get("/bridge/v1/events")
                        .query("after", after.to_string())
                        .query("limit", "500"),
                )
                .map_err(|error| provider_error("imessage", error))?
                .json()
                .map_err(|error| provider_error("imessage", error))?;
            for event in page.events {
                if event.sequence <= after {
                    return Err(ProviderError::Protocol {
                        provider: "imessage",
                        reason: "event sequence did not advance".into(),
                    });
                }
                if seen.insert(event.event_id.clone()) {
                    events.push(event);
                }
            }
            after = page.next_sequence;
            if !page.has_more {
                break;
            }
        }
        events.sort_by_key(|event| event.sequence);
        let observations = events
            .into_iter()
            .map(|event| {
                Ok(IMessageEvent {
                    bridge_id: info.bridge_id.clone().into(),
                    occurred_at: parse_millis(&event.occurred_at)?,
                    event,
                })
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        Ok(TypedPullBatch {
            observations,
            next_cursor: IMessageCursor {
                bridge_id: info.bridge_id,
                sequence: after,
            },
        })
    }

    fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error> {
        let response: ReadReceiptResponse = self
            .http
            .execute(
                HttpRequest::post_json(
                    format!(
                        "/bridge/v1/conversations/{}/read-receipts",
                        effect.conversation_id
                    ),
                    ReadReceiptRequest {
                        through_message_id: &effect.through_message_id,
                        idempotency_key: hex(&effect.idempotency.0),
                    },
                )
                .map_err(|error| provider_error("imessage", error))?,
            )
            .map_err(|error| provider_error("imessage", error))?
            .json()
            .map_err(|error| provider_error("imessage", error))?;
        let receipt = digest("imessage-receipt", response.receipt_id.as_bytes());
        Ok(match response.status {
            ReceiptStatus::Applied => EffectOutcome::Applied { receipt },
            ReceiptStatus::AlreadyApplied => EffectOutcome::AlreadyApplied { receipt },
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::connector::{EffectTarget, ProviderLocator};
    use crate::transport::MockHttp;

    use super::*;

    fn info() -> BridgeInfo {
        BridgeInfo {
            bridge_id: "phone-1".into(),
            protocol_version: 1,
            capabilities: BridgeCapabilities {
                event_backfill: true,
                push_events: true,
                read_receipts: true,
                attachments: true,
            },
        }
    }

    fn event() -> BridgeEvent {
        BridgeEvent {
            sequence: 1,
            event_id: "event-1".into(),
            occurred_at: "2026-08-22T15:00:00Z".into(),
            kind: BridgeEventKind::MessageCreated {
                message: IMessage {
                    id: "msg-1".into(),
                    conversation_id: "chat-1".into(),
                    sender: "+15551234567".into(),
                    sent_at: "2026-08-22T15:00:00Z".into(),
                    text: "Train at 12:48".into(),
                    attachments: Vec::new(),
                },
            },
        }
    }

    #[test]
    fn backfill_and_read_receipt_define_the_bridge_contract() {
        let mut http = MockHttp::new();
        http.expect_json(HttpRequest::get("/bridge/v1/info"), 200, info())
            .expect("info");
        http.expect_json(
            HttpRequest::get("/bridge/v1/events")
                .query("after", "0")
                .query("limit", "500"),
            200,
            BridgeEventPage {
                events: vec![event()],
                next_sequence: 1,
                has_more: false,
            },
        )
        .expect("events");
        let idempotency = digest("test", b"receipt");
        http.expect_json(
            HttpRequest::post_json(
                "/bridge/v1/conversations/chat-1/read-receipts",
                ReadReceiptRequest {
                    through_message_id: "msg-1",
                    idempotency_key: hex(&idempotency.0),
                },
            )
            .expect("request"),
            200,
            ReadReceiptResponse {
                receipt_id: "receipt-1".into(),
                status: ReceiptStatus::Applied,
                applied_at: "2026-08-22T15:01:00Z".into(),
            },
        )
        .expect("receipt");
        let mut connector = IMessageConnector::new(http);
        let batch = connector.pull(None).expect("pull");
        assert_eq!(batch.observations.len(), 1);
        assert_eq!(batch.next_cursor.sequence, 1);
        let request = EffectRequest {
            idempotency,
            verb: EffectVerb::SendReadReceipt,
            target: EffectTarget {
                account: digest("test", b"account"),
                object: digest("test", b"message"),
                locator: ProviderLocator::parse(
                    "imessage:conversation:chat-1:message:msg-1",
                )
                .expect("locator"),
            },
        };
        let effect = IMessageReadReceipt::try_from(request).expect("effect");
        assert!(matches!(
            connector.apply(&effect).expect("receipt"),
            EffectOutcome::Applied { .. }
        ));
        assert_eq!(connector.into_inner().finish().expect("transcript").len(), 3);
    }
}
