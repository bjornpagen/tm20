//! Hacker News Firebase API scaffold.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::connector::{
    Capability, ConnectorDescriptor, ConnectorError, ConnectorKey, CursorCodec, CursorToken,
    EffectOutcome, EffectRequest, ExternalKey, NormalizedObservation, ObservationKind, SourceClass,
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
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub by: String,
    pub time: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub descendants: u64,
    #[serde(default)]
    pub parent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HnEvent {
    pub item: HnItem,
    pub kind: ObservationKind,
}

impl From<HnEvent> for NormalizedObservation {
    fn from(event: HnEvent) -> Self {
        let id = event.item.id.to_be_bytes();
        let parent = event.item.parent.to_be_bytes();
        let time = event.item.time.to_be_bytes();
        let score = event.item.score.to_be_bytes();
        let descendants = event.item.descendants.to_be_bytes();
        let status = [u8::from(event.item.deleted), u8::from(event.item.dead)];
        let payload = serde_json::to_vec(&event.item).expect("HnItem is JSON-safe");
        Self {
            account: digest("hn-account", b"public"),
            object: digest("hn-item", id),
            thread: if event.item.parent == 0 {
                digest("hn-thread", id)
            } else {
                digest("hn-thread", parent)
            },
            event: digest_parts(
                "hn-event",
                &[&id, &time, &score, &descendants, &status],
            ),
            payload: digest("hn-payload", payload),
            kind: event.kind,
            occurred_at: event.item.time.saturating_mul(1_000),
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
        for (id, initial_kind) in candidates {
            let item: HnItem = self
                .http
                .execute(HttpRequest::get(format!("/v0/item/{id}.json")))
                .map_err(|error| provider_error("hacker-news", error))?
                .json()
                .map_err(|error| provider_error("hacker-news", error))?;
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
            item_type: "story".into(),
            by: "ada".into(),
            time: 1_700_000_000,
            title: title.into(),
            text: String::new(),
            url: format!("https://example.com/{id}"),
            score: 10,
            descendants: 2,
            parent: 0,
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
        assert_eq!(connector.into_inner().finish().expect("transcript").len(), 4);
    }
}
