//! Slack Socket Mode wake-ups plus `conversations.history` reconciliation.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, NormalizedObservation, ObservationKind, SourceClass,
    TypedConnector, TypedPullBatch,
};
use crate::providers::{ProviderError, digest, digest_parts, provider_error};
use crate::transport::{HttpRequest, HttpTransport};

const KEY: ConnectorKey = ConnectorKey::new("slack");
const CAPABILITIES: &[Capability] = &[Capability::Reconcile];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackWatermark {
    pub channel: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackCursor {
    pub watermarks: Vec<SlackWatermark>,
}

impl CursorCodec for SlackCursor {
    fn encode(&self) -> CursorToken {
        CursorToken::parse(serde_json::to_vec(self).expect("SlackCursor is JSON-safe"))
            .expect("serialized cursor is nonempty")
    }

    fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
        serde_json::from_slice(token.as_bytes())
            .map_err(|_| ConnectorError::InvalidCursor("Slack cursor is not valid JSON"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSocketEnvelope {
    pub envelope_id: String,
    pub payload: SlackEventPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackEventPayload {
    pub event_id: String,
    pub team_id: String,
    pub event: SlackMessageEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMessageEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub channel: String,
    pub user: String,
    pub text: String,
    pub ts: String,
    #[serde(default)]
    pub thread_ts: String,
    #[serde(default)]
    pub subtype: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlackHistoryResponse {
    ok: bool,
    #[serde(default)]
    messages: Vec<SlackHistoryMessage>,
    #[serde(default)]
    response_metadata: SlackResponseMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct SlackResponseMetadata {
    #[serde(default)]
    next_cursor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlackHistoryMessage {
    #[serde(rename = "type")]
    message_type: String,
    user: String,
    text: String,
    ts: String,
    #[serde(default)]
    thread_ts: String,
    #[serde(default)]
    subtype: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackEvent {
    event_id: String,
    team_id: String,
    message: SlackMessageEvent,
}

impl From<SlackEvent> for NormalizedObservation {
    fn from(event: SlackEvent) -> Self {
        let thread = if event.message.thread_ts.is_empty() {
            event.message.ts.as_str()
        } else {
            event.message.thread_ts.as_str()
        };
        let payload = serde_json::to_vec(&event.message).expect("SlackMessageEvent is JSON-safe");
        let occurred_at = slack_millis(&event.message.ts);
        let kind = match event.message.subtype.as_str() {
            "message_deleted" => ObservationKind::Deleted,
            "message_changed" => ObservationKind::Updated,
            _ if event.message.text.contains("<@") => ObservationKind::Mentioned,
            _ => ObservationKind::Created,
        };
        Self {
            account: digest("slack-team", event.team_id.as_bytes()),
            object: digest_parts(
                "slack-message",
                &[
                    event.team_id.as_bytes(),
                    event.message.channel.as_bytes(),
                    event.message.ts.as_bytes(),
                ],
            ),
            thread: digest_parts(
                "slack-thread",
                &[
                    event.team_id.as_bytes(),
                    event.message.channel.as_bytes(),
                    thread.as_bytes(),
                ],
            ),
            event: digest("slack-event", event.event_id.as_bytes()),
            payload: digest("slack-payload", payload),
            kind,
            occurred_at,
        }
    }
}

fn slack_millis(timestamp: &str) -> i64 {
    let (seconds, micros) = timestamp.split_once('.').unwrap_or((timestamp, "0"));
    let seconds = seconds.parse::<i64>().unwrap_or_default();
    let micros = micros
        .chars()
        .take(6)
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or_default();
    seconds.saturating_mul(1_000) + micros / 1_000
}

#[derive(Debug)]
pub enum SlackEffect {}

impl TryFrom<EffectRequest> for SlackEffect {
    type Error = ConnectorError;

    fn try_from(_: EffectRequest) -> Result<Self, Self::Error> {
        Err(ConnectorError::EffectForAnotherProvider)
    }
}

pub struct SlackConnector<T> {
    http: T,
    team_id: Box<str>,
    channels: Vec<Box<str>>,
    socket: VecDeque<SlackSocketEnvelope>,
    seen_events: HashSet<String>,
}

impl<T> SlackConnector<T> {
    pub fn new(
        http: T,
        team_id: impl Into<Box<str>>,
        channels: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Result<Self, ProviderError> {
        let team_id = team_id.into();
        let channels: Vec<Box<str>> = channels.into_iter().map(Into::into).collect();
        if team_id.is_empty() || channels.is_empty() || channels.iter().any(|c| c.is_empty()) {
            return Err(ProviderError::Protocol {
                provider: "slack",
                reason: "team and selected channels must be nonempty".into(),
            });
        }
        Ok(Self {
            http,
            team_id,
            channels,
            socket: VecDeque::new(),
            seen_events: HashSet::new(),
        })
    }

    pub fn push_socket(&mut self, envelope: SlackSocketEnvelope) -> Result<(), ProviderError> {
        if envelope.payload.team_id != self.team_id.as_ref() {
            return Err(ProviderError::Protocol {
                provider: "slack",
                reason: "Socket Mode event belongs to another workspace".into(),
            });
        }
        if self.seen_events.insert(envelope.payload.event_id.clone()) {
            self.socket.push_back(envelope);
        }
        Ok(())
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.http
    }
}

impl<T> TypedConnector for SlackConnector<T>
where
    T: HttpTransport + 'static,
{
    type Cursor = SlackCursor;
    type Event = SlackEvent;
    type Effect = SlackEffect;
    type Error = ProviderError;

    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            key: KEY,
            label: "Slack",
            class: SourceClass::WorkspaceChat,
            capabilities: CAPABILITIES,
        }
    }

    fn pull(
        &mut self,
        cursor: Option<&Self::Cursor>,
    ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error> {
        let mut watermarks: HashMap<String, String> = cursor
            .into_iter()
            .flat_map(|cursor| cursor.watermarks.iter())
            .map(|mark| (mark.channel.clone(), mark.timestamp.clone()))
            .collect();
        let mut observations = Vec::new();
        let mut seen_messages = HashSet::new();
        while let Some(envelope) = self.socket.pop_front() {
            let event = envelope.payload;
            let key = (event.event.channel.clone(), event.event.ts.clone());
            if seen_messages.insert(key) {
                watermarks
                    .entry(event.event.channel.clone())
                    .and_modify(|timestamp| {
                        if event.event.ts > *timestamp {
                            *timestamp = event.event.ts.clone();
                        }
                    })
                    .or_insert_with(|| event.event.ts.clone());
                observations.push(SlackEvent {
                    event_id: event.event_id,
                    team_id: event.team_id,
                    message: event.event,
                });
            }
        }

        for channel in &self.channels {
            let oldest = watermarks
                .get(channel.as_ref())
                .cloned()
                .unwrap_or_else(|| "0".into());
            let mut page = String::new();
            loop {
                let mut request = HttpRequest::get("/api/conversations.history")
                    .query("channel", channel.as_ref())
                    .query("oldest", oldest.clone())
                    .query("inclusive", "false")
                    .query("limit", "200");
                if !page.is_empty() {
                    request = request.query("cursor", page.clone());
                }
                let response: SlackHistoryResponse = self
                    .http
                    .execute(request)
                    .map_err(|error| provider_error("slack", error))?
                    .json()
                    .map_err(|error| provider_error("slack", error))?;
                if !response.ok {
                    return Err(ProviderError::Protocol {
                        provider: "slack",
                        reason: "conversations.history returned ok=false".into(),
                    });
                }
                for message in response.messages {
                    let key = (channel.to_string(), message.ts.clone());
                    if seen_messages.insert(key) {
                        let event_id = format!("history:{}:{}", channel, message.ts);
                        watermarks
                            .entry(channel.to_string())
                            .and_modify(|timestamp| {
                                if message.ts > *timestamp {
                                    *timestamp = message.ts.clone();
                                }
                            })
                            .or_insert_with(|| message.ts.clone());
                        observations.push(SlackEvent {
                            event_id,
                            team_id: self.team_id.to_string(),
                            message: SlackMessageEvent {
                                event_type: message.message_type,
                                channel: channel.to_string(),
                                user: message.user,
                                text: message.text,
                                ts: message.ts,
                                thread_ts: message.thread_ts,
                                subtype: message.subtype,
                            },
                        });
                    }
                }
                page = response.response_metadata.next_cursor;
                if page.is_empty() {
                    break;
                }
            }
        }
        let mut watermarks: Vec<SlackWatermark> = watermarks
            .into_iter()
            .map(|(channel, timestamp)| SlackWatermark { channel, timestamp })
            .collect();
        watermarks.sort_by(|a, b| a.channel.cmp(&b.channel));
        Ok(TypedPullBatch {
            observations,
            next_cursor: SlackCursor { watermarks },
        })
    }

    fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error> {
        match *effect {}
    }
}

#[cfg(test)]
mod tests {
    use crate::transport::MockHttp;

    use super::*;

    #[test]
    fn socket_is_a_hint_and_history_is_the_reliability_path() {
        let socket = SlackSocketEnvelope {
            envelope_id: "env-1".into(),
            payload: SlackEventPayload {
                event_id: "Ev-1".into(),
                team_id: "T1".into(),
                event: SlackMessageEvent {
                    event_type: "message".into(),
                    channel: "C1".into(),
                    user: "U1".into(),
                    text: "hello".into(),
                    ts: "1700000000.000100".into(),
                    thread_ts: String::new(),
                    subtype: String::new(),
                },
            },
        };
        let mut http = MockHttp::new();
        http.expect_json(
            HttpRequest::get("/api/conversations.history")
                .query("channel", "C1")
                .query("oldest", "1700000000.000100")
                .query("inclusive", "false")
                .query("limit", "200"),
            200,
            SlackHistoryResponse {
                ok: true,
                messages: Vec::new(),
                response_metadata: SlackResponseMetadata::default(),
            },
        )
        .expect("history");
        let mut connector = SlackConnector::new(http, "T1", ["C1"]).expect("connector");
        connector.push_socket(socket.clone()).expect("socket");
        connector.push_socket(socket).expect("duplicate socket");
        let batch = connector.pull(None).expect("pull");
        assert_eq!(batch.observations.len(), 1);
        assert_eq!(batch.next_cursor.watermarks.len(), 1);
        assert_eq!(connector.into_inner().finish().expect("transcript").len(), 1);
    }
}
