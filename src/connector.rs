//! Typed provider plugins with one object-safe erased boundary.
//!
//! A provider keeps its native cursor, event, effect, and error types in
//! [`TypedConnector`]. [`ErasedConnector`] is the only place those types are
//! converted into the normalized routing vocabulary consumed by
//! `Box<dyn Connector>`.

use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceClass {
    PublicFeed,
    Mail,
    WorkspaceChat,
    PersonalMessaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationKind {
    Created,
    Updated,
    Deleted,
    Mentioned,
    ReadStateChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectVerb {
    MarkRead,
    SendReadReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Reconcile,
    Effect(EffectVerb),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectorKey(&'static str);

impl ConnectorKey {
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        assert!(!key.is_empty(), "connector key is empty");
        let bytes = key.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            assert!(
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-',
                "connector key is not lowercase kebab-case"
            );
            i += 1;
        }
        Self(key)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ConnectorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectorDescriptor {
    pub key: ConnectorKey,
    pub label: &'static str,
    pub class: SourceClass,
    pub capabilities: &'static [Capability],
}

impl ConnectorDescriptor {
    #[must_use]
    pub fn supports(self, verb: EffectVerb) -> bool {
        self.capabilities.contains(&Capability::Effect(verb))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorToken(Box<[u8]>);

impl CursorToken {
    pub fn parse(bytes: impl Into<Box<[u8]>>) -> Result<Self, ConnectorError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(ConnectorError::EmptyCursor);
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

pub trait CursorCodec: Sized + Send + Sync + 'static {
    fn encode(&self) -> CursorToken;
    fn decode(token: &CursorToken) -> Result<Self, ConnectorError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalKey(pub [u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderLocator(Box<str>);

impl ProviderLocator {
    pub fn parse(value: impl Into<Box<str>>) -> Result<Self, ConnectorError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(ConnectorError::InvalidProviderLocator);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedObservation {
    pub account: ExternalKey,
    pub object: ExternalKey,
    pub thread: ExternalKey,
    pub event: ExternalKey,
    pub payload: ExternalKey,
    pub kind: ObservationKind,
    pub occurred_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedPullBatch<C, E> {
    pub observations: Vec<E>,
    pub next_cursor: C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullBatch {
    pub observations: Vec<NormalizedObservation>,
    pub next_cursor: CursorToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectTarget {
    pub account: ExternalKey,
    pub object: ExternalKey,
    pub locator: ProviderLocator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRequest {
    pub idempotency: ExternalKey,
    pub verb: EffectVerb,
    pub target: EffectTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOutcome {
    Applied { receipt: ExternalKey },
    AlreadyApplied { receipt: ExternalKey },
}

/// A provider-specific connector. Associated types prevent a Gmail cursor or
/// effect from reaching a Slack implementation before the erased boundary.
pub trait TypedConnector: Send + 'static {
    type Cursor: CursorCodec;
    type Event: Into<NormalizedObservation>;
    type Effect: TryFrom<EffectRequest, Error = ConnectorError>;
    type Error: Error + Send + Sync + 'static;

    fn descriptor(&self) -> ConnectorDescriptor;

    fn pull(
        &mut self,
        cursor: Option<&Self::Cursor>,
    ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error>;

    fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error>;
}

/// The sole runtime-polymorphic plugin vocabulary.
pub trait Connector: Send {
    fn descriptor(&self) -> ConnectorDescriptor;

    fn pull(&mut self, cursor: Option<&CursorToken>) -> Result<PullBatch, ConnectorError>;

    fn apply(&mut self, effect: EffectRequest) -> Result<EffectOutcome, ConnectorError>;
}

pub struct ErasedConnector<C>(C);

impl<C> ErasedConnector<C> {
    #[must_use]
    pub fn new(connector: C) -> Self {
        Self(connector)
    }

    #[must_use]
    pub fn into_inner(self) -> C {
        self.0
    }
}

impl<C> Connector for ErasedConnector<C>
where
    C: TypedConnector,
{
    fn descriptor(&self) -> ConnectorDescriptor {
        self.0.descriptor()
    }

    fn pull(&mut self, cursor: Option<&CursorToken>) -> Result<PullBatch, ConnectorError> {
        let cursor = cursor.map(C::Cursor::decode).transpose()?;
        let batch = self
            .0
            .pull(cursor.as_ref())
            .map_err(|error| ConnectorError::provider(self.descriptor().key, error))?;
        Ok(PullBatch {
            observations: batch.observations.into_iter().map(Into::into).collect(),
            next_cursor: batch.next_cursor.encode(),
        })
    }

    fn apply(&mut self, request: EffectRequest) -> Result<EffectOutcome, ConnectorError> {
        let descriptor = self.descriptor();
        if !descriptor.supports(request.verb) {
            return Err(ConnectorError::UnsupportedEffect {
                connector: descriptor.key,
                verb: request.verb,
            });
        }
        let effect = C::Effect::try_from(request)?;
        self.0
            .apply(&effect)
            .map_err(|error| ConnectorError::provider(descriptor.key, error))
    }
}

#[derive(Default)]
pub struct ConnectorRegistry {
    connectors: Vec<Box<dyn Connector>>,
}

impl ConnectorRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<C>(&mut self, connector: C) -> Result<(), ConnectorError>
    where
        C: TypedConnector,
    {
        self.register_erased(Box::new(ErasedConnector::new(connector)))
    }

    pub fn register_erased(&mut self, connector: Box<dyn Connector>) -> Result<(), ConnectorError> {
        let key = connector.descriptor().key;
        if self
            .connectors
            .iter()
            .any(|existing| existing.descriptor().key == key)
        {
            return Err(ConnectorError::DuplicateConnector(key));
        }
        self.connectors.push(connector);
        Ok(())
    }

    pub fn connector_mut(
        &mut self,
        key: ConnectorKey,
    ) -> Result<&mut (dyn Connector + '_), ConnectorError> {
        for connector in &mut self.connectors {
            if connector.descriptor().key == key {
                return Ok(connector.as_mut());
            }
        }
        Err(ConnectorError::UnknownConnector(key))
    }

    pub fn descriptors(&self) -> impl Iterator<Item = ConnectorDescriptor> + '_ {
        self.connectors
            .iter()
            .map(|connector| connector.descriptor())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    EmptyCursor,
    InvalidCursor(&'static str),
    DuplicateConnector(ConnectorKey),
    UnknownConnector(ConnectorKey),
    UnsupportedEffect {
        connector: ConnectorKey,
        verb: EffectVerb,
    },
    EffectForAnotherProvider,
    InvalidProviderLocator,
    Provider {
        connector: ConnectorKey,
        message: String,
    },
}

impl ConnectorError {
    fn provider(connector: ConnectorKey, error: impl Error) -> Self {
        Self::Provider {
            connector,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCursor => f.write_str("cursor is empty"),
            Self::InvalidCursor(reason) => write!(f, "invalid cursor: {reason}"),
            Self::DuplicateConnector(key) => write!(f, "connector {key} is already registered"),
            Self::UnknownConnector(key) => write!(f, "connector {key} is not registered"),
            Self::UnsupportedEffect { connector, verb } => {
                write!(f, "connector {connector} does not support {verb:?}")
            }
            Self::EffectForAnotherProvider => f.write_str("effect belongs to another provider"),
            Self::InvalidProviderLocator => f.write_str("provider locator is empty or invalid"),
            Self::Provider { connector, message } => {
                write!(f, "connector {connector}: {message}")
            }
        }
    }
}

impl Error for ConnectorError {}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: ConnectorKey = ConnectorKey::new("mail-test");
    const CAPS: &[Capability] = &[
        Capability::Reconcile,
        Capability::Effect(EffectVerb::MarkRead),
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MailCursor(u64);

    impl CursorCodec for MailCursor {
        fn encode(&self) -> CursorToken {
            CursorToken::parse(self.0.to_be_bytes().to_vec()).expect("nonempty")
        }

        fn decode(token: &CursorToken) -> Result<Self, ConnectorError> {
            let bytes: [u8; 8] = token
                .as_bytes()
                .try_into()
                .map_err(|_| ConnectorError::InvalidCursor("mail cursor is not eight bytes"))?;
            Ok(Self(u64::from_be_bytes(bytes)))
        }
    }

    #[derive(Debug)]
    struct MailError;

    impl fmt::Display for MailError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("mail failed")
        }
    }

    impl Error for MailError {}

    #[derive(Debug)]
    struct MailEffect(EffectRequest);

    impl TryFrom<EffectRequest> for MailEffect {
        type Error = ConnectorError;

        fn try_from(request: EffectRequest) -> Result<Self, Self::Error> {
            (request.verb == EffectVerb::MarkRead)
                .then_some(Self(request))
                .ok_or(ConnectorError::EffectForAnotherProvider)
        }
    }

    struct MailConnector;

    impl TypedConnector for MailConnector {
        type Cursor = MailCursor;
        type Event = NormalizedObservation;
        type Effect = MailEffect;
        type Error = MailError;

        fn descriptor(&self) -> ConnectorDescriptor {
            ConnectorDescriptor {
                key: KEY,
                label: "Mail test",
                class: SourceClass::Mail,
                capabilities: CAPS,
            }
        }

        fn pull(
            &mut self,
            cursor: Option<&Self::Cursor>,
        ) -> Result<TypedPullBatch<Self::Cursor, Self::Event>, Self::Error> {
            Ok(TypedPullBatch {
                observations: Vec::new(),
                next_cursor: MailCursor(cursor.map_or(1, |cursor| cursor.0 + 1)),
            })
        }

        fn apply(&mut self, effect: &Self::Effect) -> Result<EffectOutcome, Self::Error> {
            Ok(EffectOutcome::Applied {
                receipt: effect.0.idempotency,
            })
        }
    }

    fn request(verb: EffectVerb) -> EffectRequest {
        let key = ExternalKey([7; 32]);
        EffectRequest {
            idempotency: key,
            verb,
            target: EffectTarget {
                account: key,
                object: key,
                locator: ProviderLocator::parse("message-1").expect("locator"),
            },
        }
    }

    #[test]
    fn typed_connector_erases_once_at_the_registry() {
        let mut registry = ConnectorRegistry::new();
        registry.register(MailConnector).expect("register");
        let connector = registry.connector_mut(KEY).expect("lookup");
        let first = connector.pull(None).expect("pull");
        let second = connector.pull(Some(&first.next_cursor)).expect("reconcile");
        assert_eq!(MailCursor::decode(&second.next_cursor), Ok(MailCursor(2)));
    }

    #[test]
    fn capability_prevents_an_impossible_effect_from_entering_the_plugin() {
        let mut connector = ErasedConnector::new(MailConnector);
        assert!(matches!(
            connector.apply(request(EffectVerb::SendReadReceipt)),
            Err(ConnectorError::UnsupportedEffect { .. })
        ));
    }

    #[test]
    fn registry_rejects_ambiguous_identity() {
        let mut registry = ConnectorRegistry::new();
        registry.register(MailConnector).expect("first");
        assert!(matches!(
            registry.register(MailConnector),
            Err(ConnectorError::DuplicateConnector(KEY))
        ));
    }
}
