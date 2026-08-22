//! Google Chat `spaceEvents.list` reconciliation scaffold.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, NormalizedObservation, ObservationKind, SourceClass,
    TypedConnector, TypedPullBatch,
};
use crate::providers::{ProviderError, digest, provider_error};
use crate::transport::{HttpRequest, HttpTransport};

const KEY: ConnectorKey = ConnectorKey::new("google-chat");
const CAPABILITIES: &[Capability] = &[Capability::Reconcile];
const EPOCH: &str = "1970-01-01T00:00:00Z";
const EVENT_TYPES: &str = "google.workspace.chat.message.v1.created,google.workspace.chat.message.v1.updated,google.workspace.chat.message.v1.deleted";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatWatermark {
    pub space: String,
    pub event_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleChatCursor {
    pub watermarks: Vec<ChatWatermark>,
}

impl CursorCodec for GoogleChatCursor {
    fn encode(&self) -> CursorToken {
        CursorToken::parse(serde_json::to_vec(self).expect("GoogleChatCursor is JSON-safe"))
            .expect("serialized cursor is nonempty")
    }

    fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
        serde_json::from_slice(token.as_bytes())
            .map_err(|_| ConnectorError::InvalidCursor("Google Chat cursor is not valid JSON"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceEventsResponse {
    #[serde(default)]
    pub space_events: Vec<ChatSpaceEvent>,
    #[serde(default)]
    pub next_page_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSpaceEvent {
    pub name: String,
    pub event_time: String,
    pub event_type: String,
    pub message: ChatMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub name: String,
    pub thread: ChatThread,
    pub sender: ChatUser,
    pub create_time: String,
    pub last_update_time: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub argument_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatThread {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUser {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatEvent {
    user: Box<str>,
    event: ChatSpaceEvent,
    occurred_at: i64,
}

impl From<GoogleChatEvent> for NormalizedObservation {
    fn from(event: GoogleChatEvent) -> Self {
        let payload = serde_json::to_vec(&event.event.message).expect("ChatMessage is JSON-safe");
        let kind = match event.event.event_type.as_str() {
            "google.workspace.chat.message.v1.updated" => ObservationKind::Updated,
            "google.workspace.chat.message.v1.deleted" => ObservationKind::Deleted,
            _ if !event.event.message.argument_text.is_empty() => ObservationKind::Mentioned,
            _ => ObservationKind::Created,
        };
        Self {
            account: digest("gchat-user", event.user.as_bytes()),
            object: digest("gchat-message", event.event.message.name.as_bytes()),
            thread: digest("gchat-thread", event.event.message.thread.name.as_bytes()),
            event: digest("gchat-event", event.event.name.as_bytes()),
            payload: digest("gchat-payload", payload),
            kind,
            occurred_at: event.occurred_at,
        }
    }
}

fn parse_millis(value: &str) -> Result<i64, ProviderError> {
    let time = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| ProviderError::Protocol {
        provider: "google-chat",
        reason: format!("eventTime is not RFC3339: {error}"),
    })?;
    i64::try_from(time.unix_timestamp_nanos() / 1_000_000).map_err(|_| ProviderError::Protocol {
        provider: "google-chat",
        reason: "eventTime does not fit i64 milliseconds".into(),
    })
}

#[derive(Debug)]
pub enum GoogleChatEffect {}

impl TryFrom<EffectRequest> for GoogleChatEffect {
    type Error = ConnectorError;

    fn try_from(_: EffectRequest) -> Result<Self, Self::Error> {
        Err(ConnectorError::EffectForAnotherProvider)
    }
}

pub struct GoogleChatConnector<T> {
    http: T,
    user: Box<str>,
    spaces: Vec<Box<str>>,
}

impl<T> GoogleChatConnector<T> {
    pub fn new(
        http: T,
        user: impl Into<Box<str>>,
        spaces: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Result<Self, ProviderError> {
        let user = user.into();
        let spaces: Vec<Box<str>> = spaces.into_iter().map(Into::into).collect();
        if user.is_empty() || spaces.is_empty() || spaces.iter().any(|space| space.is_empty()) {
            return Err(ProviderError::Protocol {
                provider: "google-chat",
                reason: "user and selected spaces must be nonempty".into(),
            });
        }
        Ok(Self { http, user, spaces })
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.http
    }
}

impl<T> TypedConnector for GoogleChatConnector<T>
where
    T: HttpTransport + 'static,
{
    type Cursor = GoogleChatCursor;
    type Event = GoogleChatEvent;
    type Effect = GoogleChatEffect;
    type Error = ProviderError;

    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            key: KEY,
            label: "Google Chat",
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
            .map(|mark| (mark.space.clone(), mark.event_time.clone()))
            .collect();
        let mut observations = Vec::new();
        for space in &self.spaces {
            let after = watermarks
                .get(space.as_ref())
                .cloned()
                .unwrap_or_else(|| EPOCH.to_owned());
            let mut page = String::new();
            loop {
                let mut request = HttpRequest::get(format!("/chat/v1/{space}/spaceEvents"))
                    .query("filter", format!("eventTime > \"{after}\""))
                    .query("eventTypes", EVENT_TYPES)
                    .query("pageSize", "1000");
                if !page.is_empty() {
                    request = request.query("pageToken", page.clone());
                }
                let response: SpaceEventsResponse = self
                    .http
                    .execute(request)
                    .map_err(|error| provider_error("google-chat", error))?
                    .json()
                    .map_err(|error| provider_error("google-chat", error))?;
                for event in response.space_events {
                    let occurred_at = parse_millis(&event.event_time)?;
                    watermarks
                        .entry(space.to_string())
                        .and_modify(|time| {
                            if event.event_time > *time {
                                time.clone_from(&event.event_time);
                            }
                        })
                        .or_insert_with(|| event.event_time.clone());
                    observations.push(GoogleChatEvent {
                        user: self.user.clone(),
                        event,
                        occurred_at,
                    });
                }
                page = response.next_page_token;
                if page.is_empty() {
                    break;
                }
            }
        }
        let mut watermarks: Vec<ChatWatermark> = watermarks
            .into_iter()
            .map(|(space, event_time)| ChatWatermark { space, event_time })
            .collect();
        watermarks.sort_by(|a, b| a.space.cmp(&b.space));
        Ok(TypedPullBatch {
            observations,
            next_cursor: GoogleChatCursor { watermarks },
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
    fn space_events_are_the_recovery_contract() {
        let event = ChatSpaceEvent {
            name: "spaces/AAA/spaceEvents/1".into(),
            event_time: "2026-08-22T15:00:00Z".into(),
            event_type: "google.workspace.chat.message.v1.created".into(),
            message: ChatMessage {
                name: "spaces/AAA/messages/1".into(),
                thread: ChatThread {
                    name: "spaces/AAA/threads/1".into(),
                },
                sender: ChatUser {
                    name: "users/1".into(),
                    display_name: "Ada".into(),
                },
                create_time: "2026-08-22T15:00:00Z".into(),
                last_update_time: "2026-08-22T15:00:00Z".into(),
                text: "Build is green".into(),
                argument_text: String::new(),
            },
        };
        let mut http = MockHttp::new();
        http.expect_json(
            HttpRequest::get("/chat/v1/spaces/AAA/spaceEvents")
                .query("filter", "eventTime > \"1970-01-01T00:00:00Z\"")
                .query("eventTypes", EVENT_TYPES)
                .query("pageSize", "1000"),
            200,
            SpaceEventsResponse {
                space_events: vec![event],
                next_page_token: String::new(),
            },
        )
        .expect("events");
        let mut connector =
            GoogleChatConnector::new(http, "users/me", ["spaces/AAA"]).expect("connector");
        let batch = connector.pull(None).expect("pull");
        assert_eq!(batch.observations.len(), 1);
        assert_eq!(batch.next_cursor.watermarks.len(), 1);
        assert_eq!(
            connector.into_inner().finish().expect("transcript").len(),
            1
        );
    }
}
