//! A typed attention router: source observations become explicit notices,
//! bounded paper editions, delivery evidence, and narrowly scoped upstream
//! effects.
//!
//! The crate owns no runtime. Provider plugins retain their precise cursor,
//! event, effect, and error types behind [`connector::TypedConnector`], then a
//! blanket adapter erases only the registry boundary to [`connector::Connector`].
//! Policy is data interpreted by a small evaluator, Bumbledb is the routing
//! ledger, and tm20 is the paper projection.

pub mod connector;
pub mod delivery;
pub mod paper;
pub mod policy;
pub mod rollout;
pub mod schema;

pub use connector::{
    Capability, Connector, ConnectorDescriptor, ConnectorError, ConnectorRegistry, CursorCodec,
    CursorToken, EffectOutcome, EffectRequest, EffectTarget, EffectVerb, ErasedConnector,
    ExternalKey, NormalizedObservation, ObservationKind, PullBatch, SourceClass, TypedConnector,
    TypedPullBatch,
};
pub use delivery::{
    Ambiguous, AppliedEffect, Attempt, AttemptId, Delivered, DeliveryError, DeliveryMachine,
    Encoded, Failed, FailedEffect, Planned, ReprintPlan, ReprintReason, Transmitting,
    UpstreamEffect,
};
pub use paper::{
    CompiledEdition, Digest, DigestItem, Interrupt, MAX_DIGEST_ITEMS, MAX_TAPE_DOTS, PaperError,
    PaperText, ProjectedCopy, RenderedEdition, Section, SourceCopy, compile, render_digest,
    render_interrupt,
};
pub use policy::{
    Candidate, EffectRule, InterruptionWindow, Lane, PolicyError, PolicyTable, Privacy,
    RouteDecision, RouteRule, SenderTrust, Signal, SignalSelector, SourceSelector, TrustSelector,
    Urgency, WindowSelector, default_policy,
};
pub use rollout::{BuiltinConnector, ConnectorPlan, RolloutStage, rollout};
