//! The routing ledger.
//!
//! Source names, connector capabilities, signal names, and effect verbs are
//! ordinary records: adding a plugin is data, not a schema migration. Closed
//! relations are reserved for genuinely closed lifecycle coordinates.

bumbledb::schema! {
    pub RouterLedger;

    closed relation RouteLane as RouteLaneId = {
        Interrupt,
        NextDigest,
        CompressedDigest,
        ArchiveOnly,
    };
    closed relation PrivacyArm as PrivacyArmId = {
        MetadataOnly,
        RedactedExcerpt,
        FullExcerpt,
    };
    closed relation EditionKind as EditionKindId = { Interrupt, Digest };
    closed relation RuleScope as RuleScopeId = { AnySource, SourceCategory };
    closed relation TrustGate as TrustGateId = { AnySender, TrustedOnly };
    closed relation DeliveryPhase as DeliveryPhaseId = {
        Planned,
        Encoded,
        Transmitting,
        Delivered,
        Failed,
        Ambiguous,
    };
    closed relation ReprintReason as ReprintReasonId = {
        AfterFailure,
        AfterAmbiguity,
        DeliberateDuplicate,
    };
    closed relation EffectPhase as EffectPhaseId = { Pending, Applied, Failed };

    relation SourceCategory {
        id: u64 as SourceCategoryId, fresh,
        key: str,
        label: str,
    }
    relation Source {
        id: u64 as SourceId, fresh,
        category_ref: u64 as SourceCategoryId,
        key: str,
        label: str,
    }
    relation Account {
        id: u64 as AccountId, fresh,
        source: u64 as SourceId,
        external: bytes<32> as ExternalDigest,
        label: str,
    }
    relation SignalKind {
        id: u64 as SignalKindId, fresh,
        key: str,
        label: str,
    }
    relation EffectKind {
        id: u64 as EffectKindId, fresh,
        key: str,
        label: str,
    }

    relation Object {
        id: u64 as ObjectId, fresh,
        account: u64 as AccountId,
        external: bytes<32> as ExternalDigest,
        thread: bytes<32> as ThreadDigest,
        first_seen_at: i64,
    }
    relation IngestBatch {
        id: u64 as IngestBatchId, fresh,
        account: u64 as AccountId,
        previous: bytes<32> as CursorDigest,
        next: bytes<32> as CursorDigest,
        observed_at: i64,
    }
    relation Observation {
        id: u64 as ObservationId, fresh,
        batch: u64 as IngestBatchId,
        object: u64 as ObjectId,
        event: bytes<32> as EventDigest,
        kind: u64 as SignalKindId,
        occurred_at: i64,
        payload: bytes<32> as PayloadDigest,
    }
    relation Cursor {
        account: u64 as AccountId,
        position: bytes<32> as CursorDigest,
        payload: bytes<32> as PayloadDigest,
        observed_at: i64,
    }

    relation RouteRule {
        id: u64 as RouteRuleId, fresh,
        key: str,
        scope: u64 as RuleScopeId,
        lane: u64 as RouteLaneId,
        privacy: u64 as PrivacyArmId,
        trust: u64 as TrustGateId,
        precedence: i64,
        minimum_urgency: i64,
    }
    relation RouteRuleAny { rule: u64 as RouteRuleId }
    relation RouteRuleCategory {
        rule: u64 as RouteRuleId,
        category: u64 as SourceCategoryId,
    }
    relation RouteRuleSignal {
        rule: u64 as RouteRuleId,
        signal: u64 as SignalKindId,
    }
    relation Notice {
        id: u64 as NoticeId, fresh,
        observation: u64 as ObservationId,
        rule: u64 as RouteRuleId,
        lane: u64 as RouteLaneId,
        privacy: u64 as PrivacyArmId,
        eligible_at: i64,
        excerpt: bytes<32> as PayloadDigest,
    }

    relation Edition {
        id: u64 as EditionId, fresh,
        kind_ref: u64 as EditionKindId,
        planned_at: i64,
    }
    relation EditionNotice {
        edition: u64 as EditionId,
        notice: u64 as NoticeId,
        ordinal: u64,
    }
    relation EditionArtifact {
        edition: u64 as EditionId,
        markdown: bytes<32> as PayloadDigest,
        encoded: bytes<32> as PayloadDigest,
        height_dots: u64,
    }

    relation DeliveryAttempt {
        id: u64 as DeliveryAttemptId, fresh,
        edition: u64 as EditionId,
        ordinal: u64,
        phase: u64 as DeliveryPhaseId,
    }
    relation DeliveryPlanned { attempt: u64 as DeliveryAttemptId, at: i64 }
    relation DeliveryEncoded {
        attempt: u64 as DeliveryAttemptId,
        at: i64,
        payload: bytes<32> as PayloadDigest,
    }
    relation DeliveryTransmitting { attempt: u64 as DeliveryAttemptId, at: i64 }
    relation DeliveryDelivered { attempt: u64 as DeliveryAttemptId, at: i64 }
    relation DeliveryFailed {
        attempt: u64 as DeliveryAttemptId,
        at: i64,
        detail: bytes<32> as PayloadDigest,
    }
    relation DeliveryAmbiguous {
        attempt: u64 as DeliveryAttemptId,
        at: i64,
        detail: bytes<32> as PayloadDigest,
    }
    relation DeliveryEvent {
        attempt: u64 as DeliveryAttemptId,
        sequence: u64,
        phase: u64 as DeliveryPhaseId,
        at: i64,
    }
    relation DeliveryEventDetail {
        attempt: u64 as DeliveryAttemptId,
        sequence: u64,
        detail: bytes<32> as PayloadDigest,
    }
    relation Reprint {
        attempt: u64 as DeliveryAttemptId,
        original: u64 as DeliveryAttemptId,
        reason_ref: u64 as ReprintReasonId,
    }

    relation EffectIntent {
        id: u64 as EffectIntentId, fresh,
        notice: u64 as NoticeId,
        delivery: u64 as DeliveryAttemptId,
        kind: u64 as EffectKindId,
        phase: u64 as EffectPhaseId,
        idempotency: bytes<32> as EffectDigest,
    }
    relation EffectPending { intent: u64 as EffectIntentId, at: i64 }
    relation EffectApplied {
        intent: u64 as EffectIntentId,
        at: i64,
        receipt: bytes<32> as PayloadDigest,
    }
    relation EffectFailed {
        intent: u64 as EffectIntentId,
        at: i64,
        detail: bytes<32> as PayloadDigest,
    }
    relation EffectEvent {
        intent: u64 as EffectIntentId,
        sequence: u64,
        phase: u64 as EffectPhaseId,
        at: i64,
    }
    relation EffectEventDetail {
        intent: u64 as EffectIntentId,
        sequence: u64,
        detail: bytes<32> as PayloadDigest,
    }

    SourceCategory(key) -> SourceCategory;
    Source(key) -> Source;
    Source(category_ref) <= SourceCategory(id);
    Account(source, external) -> Account;
    Account(source) <= Source(id);
    SignalKind(key) -> SignalKind;
    EffectKind(key) -> EffectKind;

    Object(account, external) -> Object;
    Object(account) <= Account(id);
    IngestBatch(account, next) -> IngestBatch;
    IngestBatch(account) <= Account(id);
    Observation(object, event) -> Observation;
    Observation(batch) <= IngestBatch(id);
    Observation(object) <= Object(id);
    Observation(kind) <= SignalKind(id);
    Cursor(account) -> Cursor;
    Cursor(account) <= Account(id);
    Cursor(account, position) <= IngestBatch(account, next);

    RouteRule(key) -> RouteRule;
    RouteRule(scope) <= RuleScope(id);
    RouteRule(lane) <= RouteLane(id);
    RouteRule(privacy) <= PrivacyArm(id);
    RouteRule(trust) <= TrustGate(id);
    RouteRuleAny(rule) -> RouteRuleAny;
    RouteRuleAny(rule) == RouteRule(id | scope == AnySource);
    RouteRuleCategory(rule) -> RouteRuleCategory;
    RouteRuleCategory(rule) == RouteRule(id | scope == SourceCategory);
    RouteRuleCategory(category) <= SourceCategory(id);
    RouteRuleSignal(rule, signal) -> RouteRuleSignal;
    RouteRuleSignal(rule) <= RouteRule(id);
    RouteRuleSignal(signal) <= SignalKind(id);
    Notice(observation, rule) -> Notice;
    Notice(observation) <= Observation(id);
    Notice(rule) <= RouteRule(id);
    Notice(lane) <= RouteLane(id);
    Notice(privacy) <= PrivacyArm(id);

    Edition(kind_ref) <= EditionKind(id);
    EditionNotice(edition, ordinal) -> EditionNotice;
    EditionNotice(notice) -> EditionNotice;
    EditionNotice(edition) <= Edition(id);
    EditionNotice(notice) <= Notice(id);
    EditionArtifact(edition) -> EditionArtifact;
    EditionArtifact(edition) <= Edition(id);

    DeliveryAttempt(edition, ordinal) -> DeliveryAttempt;
    DeliveryAttempt(edition) <= EditionArtifact(edition);
    DeliveryAttempt(phase) <= DeliveryPhase(id);
    DeliveryPlanned(attempt) -> DeliveryPlanned;
    DeliveryPlanned(attempt) == DeliveryAttempt(id | phase == Planned);
    DeliveryEncoded(attempt) -> DeliveryEncoded;
    DeliveryEncoded(attempt) == DeliveryAttempt(id | phase == Encoded);
    DeliveryTransmitting(attempt) -> DeliveryTransmitting;
    DeliveryTransmitting(attempt) == DeliveryAttempt(id | phase == Transmitting);
    DeliveryDelivered(attempt) -> DeliveryDelivered;
    DeliveryDelivered(attempt) == DeliveryAttempt(id | phase == Delivered);
    DeliveryFailed(attempt) -> DeliveryFailed;
    DeliveryFailed(attempt) == DeliveryAttempt(id | phase == Failed);
    DeliveryAmbiguous(attempt) -> DeliveryAmbiguous;
    DeliveryAmbiguous(attempt) == DeliveryAttempt(id | phase == Ambiguous);
    DeliveryEvent(attempt, sequence) -> DeliveryEvent;
    DeliveryEvent(attempt) <= DeliveryAttempt(id);
    DeliveryEvent(phase) <= DeliveryPhase(id);
    DeliveryEventDetail(attempt, sequence) -> DeliveryEventDetail;
    DeliveryEventDetail(attempt, sequence) <= DeliveryEvent(attempt, sequence);
    Reprint(attempt) -> Reprint;
    Reprint(attempt) <= DeliveryAttempt(id);
    Reprint(original) <= DeliveryAttempt(id);
    Reprint(reason_ref) <= ReprintReason(id);
    Reprint(original | reason_ref == AfterFailure) <= DeliveryAttempt(id | phase == Failed);
    Reprint(original | reason_ref == AfterAmbiguity) <= DeliveryAttempt(id | phase == Ambiguous);
    Reprint(original | reason_ref == DeliberateDuplicate) <= DeliveryAttempt(id | phase == Delivered);

    EffectIntent(idempotency) -> EffectIntent;
    EffectIntent(notice, kind) -> EffectIntent;
    EffectIntent(notice) <= Notice(id);
    EffectIntent(delivery) <= DeliveryAttempt(id | phase == Delivered);
    EffectIntent(kind) <= EffectKind(id);
    EffectIntent(phase) <= EffectPhase(id);
    EffectPending(intent) -> EffectPending;
    EffectPending(intent) == EffectIntent(id | phase == Pending);
    EffectApplied(intent) -> EffectApplied;
    EffectApplied(intent) == EffectIntent(id | phase == Applied);
    EffectFailed(intent) -> EffectFailed;
    EffectFailed(intent) == EffectIntent(id | phase == Failed);
    EffectEvent(intent, sequence) -> EffectEvent;
    EffectEvent(intent) <= EffectIntent(id);
    EffectEvent(phase) <= EffectPhase(id);
    EffectEventDetail(intent, sequence) -> EffectEventDetail;
    EffectEventDetail(intent, sequence) <= EffectEvent(intent, sequence);
}
