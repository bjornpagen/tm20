//! Delivery as a typestate machine.
//!
//! There is no `status` field that callers can combine with unrelated
//! booleans. Each transition consumes its predecessor and only a delivered
//! attempt can mint an [`UpstreamEffect`].

use std::error::Error;
use std::fmt;

use crate::connector::{EffectRequest, EffectTarget, EffectVerb, ExternalKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttemptId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixMillis(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planned {
    pub at: UnixMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoded {
    pub at: UnixMillis,
    pub payload: ExternalKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transmitting {
    pub at: UnixMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivered {
    pub at: UnixMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failed {
    pub at: UnixMillis,
    pub detail: ExternalKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ambiguous {
    pub at: UnixMillis,
    pub detail: ExternalKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt<S> {
    pub id: AttemptId,
    pub edition: EditionId,
    pub ordinal: u64,
    pub state: S,
}

impl Attempt<Planned> {
    #[must_use]
    pub fn encoded(self, at: UnixMillis, payload: ExternalKey) -> Attempt<Encoded> {
        self.map(Encoded { at, payload })
    }

    #[must_use]
    pub fn failed(self, at: UnixMillis, detail: ExternalKey) -> Attempt<Failed> {
        self.map(Failed { at, detail })
    }
}

impl Attempt<Encoded> {
    #[must_use]
    pub fn transmitting(self, at: UnixMillis) -> Attempt<Transmitting> {
        self.map(Transmitting { at })
    }

    #[must_use]
    pub fn failed(self, at: UnixMillis, detail: ExternalKey) -> Attempt<Failed> {
        self.map(Failed { at, detail })
    }
}

impl Attempt<Transmitting> {
    #[must_use]
    pub fn delivered(self, at: UnixMillis) -> Attempt<Delivered> {
        self.map(Delivered { at })
    }

    #[must_use]
    pub fn ambiguous(self, at: UnixMillis, detail: ExternalKey) -> Attempt<Ambiguous> {
        self.map(Ambiguous { at, detail })
    }
}

impl<S> Attempt<S> {
    fn map<T>(self, state: T) -> Attempt<T> {
        Attempt {
            id: self.id,
            edition: self.edition,
            ordinal: self.ordinal,
            state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamEffect {
    pub delivered_by: AttemptId,
    pub request: EffectRequest,
}

impl UpstreamEffect {
    #[must_use]
    pub fn applied(self, at: UnixMillis, receipt: ExternalKey) -> AppliedEffect {
        AppliedEffect {
            effect: self,
            at,
            receipt,
        }
    }

    #[must_use]
    pub fn failed(self, at: UnixMillis, detail: ExternalKey) -> FailedEffect {
        FailedEffect {
            effect: self,
            at,
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEffect {
    pub effect: UpstreamEffect,
    pub at: UnixMillis,
    pub receipt: ExternalKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedEffect {
    pub effect: UpstreamEffect,
    pub at: UnixMillis,
    pub detail: ExternalKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReprintReason {
    AfterFailure,
    AfterAmbiguity,
    DeliberateDuplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReprintPlan {
    pub attempt: Attempt<Planned>,
    pub original: AttemptId,
    pub reason: ReprintReason,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeliveryMachine;

impl DeliveryMachine {
    #[must_use]
    pub fn plan(
        id: AttemptId,
        edition: EditionId,
        ordinal: u64,
        at: UnixMillis,
    ) -> Attempt<Planned> {
        Attempt {
            id,
            edition,
            ordinal,
            state: Planned { at },
        }
    }

    #[must_use]
    pub fn release_effect(
        delivered: &Attempt<Delivered>,
        idempotency: ExternalKey,
        verb: EffectVerb,
        target: EffectTarget,
    ) -> UpstreamEffect {
        UpstreamEffect {
            delivered_by: delivered.id,
            request: EffectRequest {
                idempotency,
                verb,
                target,
            },
        }
    }

    #[must_use]
    pub fn reprint_after_failure(
        original: &Attempt<Failed>,
        id: AttemptId,
        ordinal: u64,
        at: UnixMillis,
    ) -> ReprintPlan {
        Self::reprint(
            original.id,
            original.edition,
            id,
            ordinal,
            at,
            ReprintReason::AfterFailure,
        )
    }

    #[must_use]
    pub fn reprint_after_ambiguity(
        original: &Attempt<Ambiguous>,
        id: AttemptId,
        ordinal: u64,
        at: UnixMillis,
    ) -> ReprintPlan {
        Self::reprint(
            original.id,
            original.edition,
            id,
            ordinal,
            at,
            ReprintReason::AfterAmbiguity,
        )
    }

    #[must_use]
    pub fn deliberate_reprint(
        original: &Attempt<Delivered>,
        id: AttemptId,
        ordinal: u64,
        at: UnixMillis,
    ) -> ReprintPlan {
        Self::reprint(
            original.id,
            original.edition,
            id,
            ordinal,
            at,
            ReprintReason::DeliberateDuplicate,
        )
    }

    fn reprint(
        original: AttemptId,
        edition: EditionId,
        id: AttemptId,
        ordinal: u64,
        at: UnixMillis,
        reason: ReprintReason,
    ) -> ReprintPlan {
        ReprintPlan {
            attempt: Self::plan(id, edition, ordinal, at),
            original,
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryError {
    ReprintRequiresNewAttempt,
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReprintRequiresNewAttempt => {
                f.write_str("a reprint is a new labeled attempt, never a retry mutation")
            }
        }
    }
}

impl Error for DeliveryError {}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO: ExternalKey = ExternalKey([0; 32]);
    const ONE: ExternalKey = ExternalKey([1; 32]);

    fn target() -> EffectTarget {
        EffectTarget {
            account: ZERO,
            object: ONE,
        }
    }

    #[test]
    fn the_success_path_consumes_each_predecessor() {
        let planned = DeliveryMachine::plan(AttemptId(1), EditionId(9), 0, UnixMillis(10));
        let encoded = planned.encoded(UnixMillis(11), ONE);
        let transmitting = encoded.transmitting(UnixMillis(12));
        let delivered = transmitting.delivered(UnixMillis(13));
        let effect =
            DeliveryMachine::release_effect(&delivered, ZERO, EffectVerb::MarkRead, target());
        assert_eq!(effect.delivered_by, AttemptId(1));
        let applied = effect.applied(UnixMillis(14), ONE);
        assert_eq!(applied.receipt, ONE);
    }

    #[test]
    fn a_post_transmission_disconnect_has_only_the_ambiguous_coordinate() {
        let ambiguous = DeliveryMachine::plan(AttemptId(1), EditionId(9), 0, UnixMillis(10))
            .encoded(UnixMillis(11), ONE)
            .transmitting(UnixMillis(12))
            .ambiguous(UnixMillis(13), ZERO);
        assert_eq!(ambiguous.state.detail, ZERO);
    }

    #[test]
    fn an_ambiguous_delivery_can_only_be_reprinted_as_a_new_labeled_attempt() {
        let original = DeliveryMachine::plan(AttemptId(1), EditionId(9), 0, UnixMillis(10))
            .encoded(UnixMillis(11), ONE)
            .transmitting(UnixMillis(12))
            .ambiguous(UnixMillis(13), ZERO);
        let reprint =
            DeliveryMachine::reprint_after_ambiguity(&original, AttemptId(2), 1, UnixMillis(14));
        assert_eq!(reprint.original, AttemptId(1));
        assert_eq!(reprint.attempt.id, AttemptId(2));
        assert_eq!(reprint.reason, ReprintReason::AfterAmbiguity);
    }
}
