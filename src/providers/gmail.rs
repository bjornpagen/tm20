//! Gmail API scaffold: bounded initial sync, `historyId` reconciliation, and
//! idempotent `UNREAD` label removal.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, EffectVerb, ExternalKey, NormalizedObservation, ObservationKind,
    SourceClass, TypedConnector, TypedPullBatch,
};
use crate::providers::{ProviderError, digest, digest_parts, provider_error};
use crate::transport::{HttpRequest, HttpTransport};

const KEY: ConnectorKey = ConnectorKey::new("gmail");
const CAPABILITIES: &[Capability] = &[
    Capability::Reconcile,
    Capability::Effect(EffectVerb::MarkRead),
];
const INITIAL_LIMIT: u16 = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailAccount(Box<str>);

impl GmailAccount {
    pub fn parse(value: impl Into<Box<str>>) -> Result<Self, ProviderError> {
        let value = value.into();
        if value.contains('@') && !value.chars().any(char::is_control) {
            Ok(Self(value))
        } else {
            Err(ProviderError::Protocol {
                provider: "gmail",
                reason: "account is not an email address".into(),
            })
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailCursor {
    pub history_id: String,
}

impl CursorCodec for GmailCursor {
    fn encode(&self) -> CursorToken {
        CursorToken::parse(serde_json::to_vec(self).expect("GmailCursor is JSON-safe"))
            .expect("serialized cursor is nonempty")
    }

    fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
        let cursor: Self = serde_json::from_slice(token.as_bytes())
            .map_err(|_| ConnectorError::InvalidCursor("Gmail cursor is not valid JSON"))?;
        if cursor.history_id.is_empty() {
            return Err(ConnectorError::InvalidCursor(
                "Gmail historyId must not be empty",
            ));
        }
        Ok(cursor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailProfile {
    pub email_address: String,
    pub history_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessageList {
    #[serde(default)]
    pub messages: Vec<GmailMessageRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailHistoryResponse {
    #[serde(default)]
    pub history: Vec<GmailHistory>,
    pub history_id: String,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailHistory {
    pub id: String,
    #[serde(default)]
    pub messages_added: Vec<GmailMessageAdded>,
    #[serde(default)]
    pub messages_deleted: Vec<GmailMessageAdded>,
    #[serde(default)]
    pub labels_added: Vec<GmailLabelChange>,
    #[serde(default)]
    pub labels_removed: Vec<GmailLabelChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessageAdded {
    pub message: GmailMessageRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailLabelChange {
    pub message: GmailMessageRef,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessageRef {
    pub id: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub history_id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    pub internal_date: String,
    pub payload: GmailPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailPayload {
    #[serde(default)]
    pub headers: Vec<GmailHeader>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailEvent {
    account: GmailAccount,
    history_id: String,
    pub message: GmailMessage,
    pub change: GmailChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "labels", rename_all = "snake_case")]
pub enum GmailChange {
    Added,
    Deleted,
    LabelsAdded(Vec<String>),
    LabelsRemoved(Vec<String>),
}

impl From<GmailEvent> for NormalizedObservation {
    fn from(event: GmailEvent) -> Self {
        let payload =
            serde_json::to_vec(&(&event.message, &event.change)).expect("Gmail event is JSON-safe");
        let occurred_at = event
            .message
            .internal_date
            .parse::<i64>()
            .unwrap_or_default();
        let (kind, change) = match &event.change {
            GmailChange::Added => (ObservationKind::Created, b"added".as_slice()),
            GmailChange::Deleted => (ObservationKind::Deleted, b"deleted".as_slice()),
            GmailChange::LabelsAdded(labels) => {
                let kind = if labels.iter().any(|label| label == "UNREAD") {
                    ObservationKind::ReadStateChanged
                } else {
                    ObservationKind::Updated
                };
                (kind, b"labels-added".as_slice())
            }
            GmailChange::LabelsRemoved(labels) => {
                let kind = if labels.iter().any(|label| label == "UNREAD") {
                    ObservationKind::ReadStateChanged
                } else {
                    ObservationKind::Updated
                };
                (kind, b"labels-removed".as_slice())
            }
        };
        Self {
            account: digest("gmail-account", event.account.as_str()),
            object: digest_parts(
                "gmail-message",
                &[
                    event.account.as_str().as_bytes(),
                    event.message.id.as_bytes(),
                ],
            ),
            thread: digest_parts(
                "gmail-thread",
                &[
                    event.account.as_str().as_bytes(),
                    event.message.thread_id.as_bytes(),
                ],
            ),
            event: digest_parts(
                "gmail-event",
                &[
                    event.history_id.as_bytes(),
                    change,
                    event.message.id.as_bytes(),
                    &payload,
                ],
            ),
            payload: digest("gmail-payload", payload),
            kind,
            occurred_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailEffect {
    pub message_id: Box<str>,
    pub idempotency: ExternalKey,
}

impl TryFrom<EffectRequest> for GmailEffect {
    type Error = ConnectorError;

    fn try_from(request: EffectRequest) -> Result<Self, Self::Error> {
        if request.verb != EffectVerb::MarkRead {
            return Err(ConnectorError::EffectForAnotherProvider);
        }
        let message_id = request
            .target
            .locator
            .as_str()
            .strip_prefix("gmail:message:")
            .filter(|id| !id.is_empty())
            .ok_or(ConnectorError::EffectForAnotherProvider)?;
        Ok(Self {
            message_id: message_id.into(),
            idempotency: request.idempotency,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyMessage {
    pub remove_label_ids: Vec<String>,
    pub add_label_ids: Vec<String>,
}

pub struct GmailConnector<T> {
    http: T,
    account: GmailAccount,
}

impl<T> GmailConnector<T> {
    pub fn new(http: T, account: GmailAccount) -> Self {
        Self { http, account }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.http
    }
}

impl<T> GmailConnector<T>
where
    T: HttpTransport,
{
    fn fetch_message(&mut self, id: &str) -> Result<GmailMessage, ProviderError> {
        self.http
            .execute(
                HttpRequest::get(format!("/gmail/v1/users/me/messages/{id}"))
                    .query("format", "METADATA")
                    .query("metadataHeaders", "From")
                    .query("metadataHeaders", "To")
                    .query("metadataHeaders", "Cc")
                    .query("metadataHeaders", "Subject")
                    .query("metadataHeaders", "Date")
                    .query("metadataHeaders", "Message-ID"),
            )
            .map_err(|error| provider_error("gmail", error))?
            .json()
            .map_err(|error| provider_error("gmail", error))
    }

    fn initial_pull(&mut self) -> Result<TypedPullBatch<GmailCursor, GmailEvent>, ProviderError> {
        let profile: GmailProfile = self
            .http
            .execute(HttpRequest::get("/gmail/v1/users/me/profile"))
            .map_err(|error| provider_error("gmail", error))?
            .json()
            .map_err(|error| provider_error("gmail", error))?;
        if profile.email_address != self.account.as_str() {
            return Err(ProviderError::Protocol {
                provider: "gmail",
                reason: "profile email does not match configured account".into(),
            });
        }
        let list: GmailMessageList = self
            .http
            .execute(
                HttpRequest::get("/gmail/v1/users/me/messages")
                    .query("labelIds", "INBOX")
                    .query("maxResults", INITIAL_LIMIT.to_string()),
            )
            .map_err(|error| provider_error("gmail", error))?
            .json()
            .map_err(|error| provider_error("gmail", error))?;
        let mut observations = Vec::with_capacity(list.messages.len());
        for reference in list.messages {
            let message = self.fetch_message(&reference.id)?;
            observations.push(GmailEvent {
                account: self.account.clone(),
                history_id: profile.history_id.clone(),
                message,
                change: GmailChange::Added,
            });
        }
        Ok(TypedPullBatch {
            observations,
            next_cursor: GmailCursor {
                history_id: profile.history_id,
            },
        })
    }

    fn history_pull(
        &mut self,
        cursor: &GmailCursor,
    ) -> Result<TypedPullBatch<GmailCursor, GmailEvent>, ProviderError> {
        let mut page_token = None;
        let mut message_ids = Vec::new();
        let mut seen = HashSet::new();
        let latest_history = loop {
            let mut request = HttpRequest::get("/gmail/v1/users/me/history")
                .query("startHistoryId", cursor.history_id.clone())
                .query("maxResults", "500")
                .query("historyTypes", "messageAdded")
                .query("historyTypes", "messageDeleted")
                .query("historyTypes", "labelAdded")
                .query("historyTypes", "labelRemoved");
            if let Some(token) = &page_token {
                request = request.query("pageToken", token);
            }
            let page: GmailHistoryResponse = self
                .http
                .execute(request)
                .map_err(|error| provider_error("gmail", error))?
                .json()
                .map_err(|error| provider_error("gmail", error))?;
            let page_history = page.history_id;
            for history in page.history {
                let mut push = |reference: GmailMessageRef, change: GmailChange| {
                    let key = (reference.id.clone(), change.clone());
                    if seen.insert(key) {
                        message_ids.push((history.id.clone(), reference.id, change));
                    }
                };
                for added in history.messages_added {
                    push(added.message, GmailChange::Added);
                }
                for deleted in history.messages_deleted {
                    push(deleted.message, GmailChange::Deleted);
                }
                for labels in history.labels_added {
                    let mut label_ids = labels.label_ids;
                    label_ids.sort();
                    push(labels.message, GmailChange::LabelsAdded(label_ids));
                }
                for labels in history.labels_removed {
                    let mut label_ids = labels.label_ids;
                    label_ids.sort();
                    push(labels.message, GmailChange::LabelsRemoved(label_ids));
                }
            }
            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => break page_history,
            }
        };
        let mut observations = Vec::with_capacity(message_ids.len());
        for (history_id, message_id, change) in message_ids {
            let message = self.fetch_message(&message_id)?;
            observations.push(GmailEvent {
                account: self.account.clone(),
                history_id,
                message,
                change,
            });
        }
        Ok(TypedPullBatch {
            observations,
            next_cursor: GmailCursor {
                history_id: latest_history,
            },
        })
    }
}

impl<T> TypedConnector for GmailConnector<T>
where
    T: HttpTransport + 'static,
{
    type Cursor = GmailCursor;
    type Event = GmailEvent;
    type Effect = GmailEffect;
    type Error = ProviderError;

    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            key: KEY,
            label: "Gmail",
            class: SourceClass::Mail,
            capabilities: CAPABILITIES,
        }
    }

    fn pull(
        &mut self,
        cursor: Option<&Self::Cursor>,
    ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error> {
        match cursor {
            Some(cursor) => self.history_pull(cursor),
            None => self.initial_pull(),
        }
    }

    fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error> {
        let message: GmailMessage = self
            .http
            .execute(
                HttpRequest::post_json(
                    format!("/gmail/v1/users/me/messages/{}/modify", effect.message_id),
                    ModifyMessage {
                        remove_label_ids: vec!["UNREAD".into()],
                        add_label_ids: Vec::new(),
                    },
                )
                .map_err(|error| provider_error("gmail", error))?,
            )
            .map_err(|error| provider_error("gmail", error))?
            .json()
            .map_err(|error| provider_error("gmail", error))?;
        Ok(EffectOutcome::Applied {
            receipt: digest_parts(
                "gmail-mark-read",
                &[&effect.idempotency.0, message.id.as_bytes()],
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::connector::{EffectTarget, ProviderLocator};
    use crate::transport::MockHttp;

    use super::*;

    fn message() -> GmailMessage {
        GmailMessage {
            id: "18d00a".into(),
            thread_id: "thread-1".into(),
            history_id: "101".into(),
            label_ids: vec!["INBOX".into(), "UNREAD".into()],
            internal_date: "1700000000000".into(),
            payload: GmailPayload {
                headers: vec![
                    GmailHeader {
                        name: "From".into(),
                        value: "Ada <ada@example.com>".into(),
                    },
                    GmailHeader {
                        name: "Subject".into(),
                        value: "Status".into(),
                    },
                ],
            },
        }
    }

    #[test]
    fn history_and_mark_read_follow_gmail_api_shapes() {
        let cursor = GmailCursor {
            history_id: "100".into(),
        };
        let mut http = MockHttp::new();
        http.expect_json(
            HttpRequest::get("/gmail/v1/users/me/history")
                .query("startHistoryId", "100")
                .query("maxResults", "500")
                .query("historyTypes", "messageAdded")
                .query("historyTypes", "messageDeleted")
                .query("historyTypes", "labelAdded")
                .query("historyTypes", "labelRemoved"),
            200,
            GmailHistoryResponse {
                history: vec![GmailHistory {
                    id: "101".into(),
                    messages_added: vec![GmailMessageAdded {
                        message: GmailMessageRef {
                            id: "18d00a".into(),
                            thread_id: "thread-1".into(),
                        },
                    }],
                    messages_deleted: Vec::new(),
                    labels_added: Vec::new(),
                    labels_removed: Vec::new(),
                }],
                history_id: "101".into(),
                next_page_token: None,
            },
        )
        .expect("history");
        http.expect_json(
            HttpRequest::get("/gmail/v1/users/me/messages/18d00a")
                .query("format", "METADATA")
                .query("metadataHeaders", "From")
                .query("metadataHeaders", "To")
                .query("metadataHeaders", "Cc")
                .query("metadataHeaders", "Subject")
                .query("metadataHeaders", "Date")
                .query("metadataHeaders", "Message-ID"),
            200,
            message(),
        )
        .expect("message");
        let read = GmailMessage {
            label_ids: vec!["INBOX".into()],
            ..message()
        };
        http.expect_json(
            HttpRequest::post_json(
                "/gmail/v1/users/me/messages/18d00a/modify",
                ModifyMessage {
                    remove_label_ids: vec!["UNREAD".into()],
                    add_label_ids: Vec::new(),
                },
            )
            .expect("request"),
            200,
            read,
        )
        .expect("modify");

        let account = GmailAccount::parse("me@example.com").expect("account");
        let mut connector = GmailConnector::new(http, account);
        let batch = connector.pull(Some(&cursor)).expect("pull");
        assert_eq!(batch.next_cursor.history_id, "101");
        assert_eq!(batch.observations.len(), 1);
        let key = digest("test", b"effect");
        let request = EffectRequest {
            idempotency: key,
            verb: EffectVerb::MarkRead,
            target: EffectTarget {
                account: key,
                object: key,
                locator: ProviderLocator::parse("gmail:message:18d00a").expect("locator"),
            },
        };
        let effect = GmailEffect::try_from(request).expect("effect");
        assert!(matches!(
            connector.apply(&effect).expect("apply"),
            EffectOutcome::Applied { .. }
        ));
        assert_eq!(
            connector.into_inner().finish().expect("transcript").len(),
            3
        );
    }
}
