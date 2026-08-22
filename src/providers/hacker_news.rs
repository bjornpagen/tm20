//! Hacker News Firebase API scaffold.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, NormalizedObservation, ObservationKind, SourceClass,
    TypedConnector, TypedPullBatch,
};
use crate::providers::{ProviderError, digest, digest_parts, provider_error};
use crate::transport::{HttpRequest, HttpTransport};

const KEY: ConnectorKey = ConnectorKey::new("hacker-news");
const CAPABILITIES: &[Capability] = &[Capability::Reconcile];
const DEFAULT_RANK_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HnCursor {
    pub max_item: u64,
    pub ranked: Vec<u64>,
    pub pending_null: Vec<u64>,
}

impl CursorCodec for HnCursor {
    fn encode(&self) -> CursorToken {
        CursorToken::parse(serde_json::to_vec(self).expect("HnCursor is JSON-safe"))
            .expect("serialized cursor is nonempty")
    }

    fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
        serde_json::from_slice(token.as_bytes())
            .map_err(|_| ConnectorError::InvalidCursor("Hacker News cursor is not valid JSON"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HnItem {
    pub id: u64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub dead: bool,
    #[serde(rename = "type", default)]
    pub item_type: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(default)]
    pub descendants: Option<u64>,
    #[serde(default)]
    pub parent: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HnEvent {
    pub item: HnItem,
    pub kind: ObservationKind,
}

impl From<HnEvent> for NormalizedObservation {
    fn from(event: HnEvent) -> Self {
        let id = event.item.id.to_be_bytes();
        let parent = event.item.parent.unwrap_or(event.item.id).to_be_bytes();
        let time = event.item.time.unwrap_or_default().to_be_bytes();
        let score = event.item.score.unwrap_or_default().to_be_bytes();
        let descendants = event.item.descendants.unwrap_or_default().to_be_bytes();
        let status = [u8::from(event.item.deleted), u8::from(event.item.dead)];
        let payload = serde_json::to_vec(&event.item).expect("HnItem is JSON-safe");
        Self {
            account: digest("hn-account", b"public"),
            object: digest("hn-item", id),
            thread: digest("hn-thread", parent),
            event: digest_parts("hn-event", &[&id, &time, &score, &descendants, &status]),
            payload: digest("hn-payload", payload),
            kind: event.kind,
            occurred_at: if event.kind == ObservationKind::Created {
                event.item.time.unwrap_or_default().saturating_mul(1_000)
            } else {
                0
            },
        }
    }
}

#[derive(Debug)]
pub enum HnEffect {}

impl TryFrom<EffectRequest> for HnEffect {
    type Error = ConnectorError;

    fn try_from(_: EffectRequest) -> Result<Self, Self::Error> {
        Err(ConnectorError::EffectForAnotherProvider)
    }
}

pub struct HackerNewsConnector<T> {
    http: T,
    rank_limit: usize,
}

impl<T> HackerNewsConnector<T> {
    pub fn new(http: T) -> Self {
        Self {
            http,
            rank_limit: DEFAULT_RANK_LIMIT,
        }
    }

    #[must_use]
    pub fn with_rank_limit(mut self, rank_limit: usize) -> Self {
        self.rank_limit = rank_limit.max(1);
        self
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.http
    }
}

impl<T> TypedConnector for HackerNewsConnector<T>
where
    T: HttpTransport + 'static,
{
    type Cursor = HnCursor;
    type Event = HnEvent;
    type Effect = HnEffect;
    type Error = ProviderError;

    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            key: KEY,
            label: "Hacker News",
            class: SourceClass::PublicFeed,
            capabilities: CAPABILITIES,
        }
    }

    fn pull(
        &mut self,
        cursor: Option<&Self::Cursor>,
    ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error> {
        let max_item: u64 = self
            .http
            .execute(HttpRequest::get("/v0/maxitem.json"))
            .map_err(|error| provider_error("hacker-news", error))?
            .json()
            .map_err(|error| provider_error("hacker-news", error))?;
        let ranked: Vec<u64> = self
            .http
            .execute(HttpRequest::get("/v0/topstories.json"))
            .map_err(|error| provider_error("hacker-news", error))?
            .json()
            .map_err(|error| provider_error("hacker-news", error))?;
        let ranked: Vec<u64> = ranked.into_iter().take(self.rank_limit).collect();

        let previous_max = cursor.map_or(max_item, |cursor| cursor.max_item);
        let previous_ranked: HashSet<u64> = cursor
            .map(|cursor| cursor.ranked.iter().copied().collect())
            .unwrap_or_default();
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        if let Some(cursor) = cursor {
            for id in &cursor.pending_null {
                if seen.insert(*id) {
                    candidates.push((*id, ObservationKind::Updated));
                }
            }
        }
        for id in &ranked {
            if (cursor.is_none() || !previous_ranked.contains(id)) && seen.insert(*id) {
                let kind = if cursor.is_none() {
                    ObservationKind::Created
                } else {
                    ObservationKind::Updated
                };
                candidates.push((*id, kind));
            }
        }
        if cursor.is_some() {
            for id in previous_max.saturating_add(1)..=max_item {
                if seen.insert(id) {
                    candidates.push((id, ObservationKind::Created));
                }
            }
        }

        let mut observations = Vec::with_capacity(candidates.len());
        let mut pending_null = Vec::new();
        for (id, initial_kind) in candidates {
            let item: Option<HnItem> = self
                .http
                .execute(HttpRequest::get(format!("/v0/item/{id}.json")))
                .map_err(|error| provider_error("hacker-news", error))?
                .json()
                .map_err(|error| provider_error("hacker-news", error))?;
            let Some(item) = item else {
                pending_null.push(id);
                continue;
            };
            if item.id != id {
                return Err(ProviderError::Protocol {
                    provider: "hacker-news",
                    reason: format!("requested item {id}, received {}", item.id),
                });
            }
            let kind = if item.deleted || item.dead {
                ObservationKind::Deleted
            } else {
                initial_kind
            };
            observations.push(HnEvent { item, kind });
        }

        Ok(TypedPullBatch {
            observations,
            next_cursor: HnCursor {
                max_item,
                ranked,
                pending_null,
            },
        })
    }

    fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error> {
        match *effect {}
    }
}

#[cfg(test)]
mod tests {
    use crate::connector::TypedConnector;
    use crate::transport::MockHttp;

    use super::*;

    fn item(id: u64, title: &str) -> HnItem {
        HnItem {
            id,
            deleted: false,
            dead: false,
            item_type: Some("story".into()),
            by: Some("ada".into()),
            time: Some(1_700_000_000),
            title: Some(title.into()),
            text: None,
            url: Some(format!("https://example.com/{id}")),
            score: Some(10),
            descendants: Some(2),
            parent: None,
        }
    }

    #[test]
    fn initial_pull_uses_real_hn_endpoint_shapes() {
        let mut http = MockHttp::new();
        http.expect_json(HttpRequest::get("/v0/maxitem.json"), 200, 42u64)
            .expect("maxitem");
        http.expect_json(
            HttpRequest::get("/v0/topstories.json"),
            200,
            vec![42u64, 41],
        )
        .expect("top");
        http.expect_json(
            HttpRequest::get("/v0/item/42.json"),
            200,
            item(42, "Typed plugins"),
        )
        .expect("42");
        http.expect_json(
            HttpRequest::get("/v0/item/41.json"),
            200,
            item(41, "Paper inboxes"),
        )
        .expect("41");

        let mut connector = HackerNewsConnector::new(http);
        let batch = connector.pull(None).expect("pull");
        assert_eq!(batch.observations.len(), 2);
        assert_eq!(batch.next_cursor.max_item, 42);
        assert!(batch.next_cursor.pending_null.is_empty());
        assert_eq!(
            connector.into_inner().finish().expect("transcript").len(),
            4
        );
    }

    #[test]
    fn null_items_remain_in_the_durable_retry_set() {
        let mut http = MockHttp::new();
        http.expect_json(HttpRequest::get("/v0/maxitem.json"), 200, 42u64)
            .expect("maxitem");
        http.expect_json(HttpRequest::get("/v0/topstories.json"), 200, vec![42u64])
            .expect("top");
        http.expect_json(
            HttpRequest::get("/v0/item/41.json"),
            200,
            Option::<HnItem>::None,
        )
        .expect("pending null");
        http.expect_json(
            HttpRequest::get("/v0/item/42.json"),
            200,
            Some(item(42, "New story")),
        )
        .expect("42");
        let cursor = HnCursor {
            max_item: 40,
            ranked: vec![40],
            pending_null: vec![41],
        };
        let mut connector = HackerNewsConnector::new(http);
        let batch = connector.pull(Some(&cursor)).expect("pull");
        assert_eq!(batch.observations.len(), 1);
        assert_eq!(batch.next_cursor.pending_null, vec![41]);
    }
}
