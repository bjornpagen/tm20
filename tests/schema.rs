use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bumbledb::{Admission, Db, WriteTx};
use paper_attention_router::schema::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempStore(PathBuf);

impl TempStore {
    fn new(name: &str) -> Self {
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "paper-router-{name}-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Clone, Copy)]
struct Seed {
    account: AccountId,
    observation: ObservationId,
    notice: NoticeId,
    edition: EditionId,
    effect_kind: EffectKindId,
}

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn seed_ingest(tx: &mut WriteTx<'_, RouterLedger>) -> bumbledb::Result<(AccountId, ObservationId)> {
    let category: SourceCategoryId = tx.reserve(1)?.start().expect("category");
    tx.insert([&SourceCategory {
        id: category,
        key: "mail",
        label: "Mail",
    }])?;
    let source: SourceId = tx.reserve(1)?.start().expect("source");
    tx.insert([&Source {
        id: source,
        category_ref: category,
        key: "gmail",
        label: "Gmail",
    }])?;
    let account: AccountId = tx.reserve(1)?.start().expect("account");
    tx.insert([&Account {
        id: account,
        source,
        external: ExternalDigest(digest(1)),
        label: "personal",
    }])?;
    let signal: SignalKindId = tx.reserve(1)?.start().expect("signal");
    tx.insert([&SignalKind {
        id: signal,
        key: "created",
        label: "Created",
    }])?;
    let object: ObjectId = tx.reserve(1)?.start().expect("object");
    tx.insert([&Object {
        id: object,
        account,
        external: ExternalDigest(digest(2)),
        thread: ThreadDigest(digest(3)),
        first_seen_at: 10,
    }])?;
    let batch: IngestBatchId = tx.reserve(1)?.start().expect("batch");
    tx.insert([&IngestBatch {
        id: batch,
        account,
        previous: CursorDigest(digest(4)),
        next: CursorDigest(digest(5)),
        observed_at: 11,
    }])?;
    let observation: ObservationId = tx.reserve(1)?.start().expect("observation");
    tx.insert([&Observation {
        id: observation,
        batch,
        object,
        event: EventDigest(digest(6)),
        kind: signal,
        occurred_at: 10,
        payload: PayloadDigest(digest(7)),
    }])?;
    tx.insert([&Cursor {
        account,
        position: CursorDigest(digest(5)),
        payload: PayloadDigest(digest(8)),
        observed_at: 11,
    }])?;
    Ok((account, observation))
}

fn seed_projection(
    tx: &mut WriteTx<'_, RouterLedger>,
    observation: ObservationId,
) -> bumbledb::Result<(NoticeId, EditionId, EffectKindId)> {
    let rule: RouteRuleId = tx.reserve(1)?.start().expect("rule");
    tx.insert([&RouteRule {
        id: rule,
        key: "mail-attention",
        scope: RuleScope::AnySource.id(),
        lane: RouteLane::NextDigest.id(),
        privacy: PrivacyArm::MetadataOnly.id(),
        trust: TrustGate::AnySender.id(),
        precedence: 10,
        minimum_urgency: 1,
    }])?;
    tx.insert([&RouteRuleAny { rule }])?;
    let notice: NoticeId = tx.reserve(1)?.start().expect("notice");
    tx.insert([&Notice {
        id: notice,
        observation,
        rule,
        lane: RouteLane::NextDigest.id(),
        privacy: PrivacyArm::MetadataOnly.id(),
        eligible_at: 12,
        excerpt: PayloadDigest(digest(9)),
    }])?;
    let edition: EditionId = tx.reserve(1)?.start().expect("edition");
    tx.insert([&Edition {
        id: edition,
        kind_ref: EditionKind::Digest.id(),
        planned_at: 13,
    }])?;
    tx.insert([&EditionNotice {
        edition,
        notice,
        ordinal: 0,
    }])?;
    tx.insert([&EditionArtifact {
        edition,
        markdown: PayloadDigest(digest(10)),
        encoded: PayloadDigest(digest(11)),
        height_dots: 500,
    }])?;
    let effect_kind: EffectKindId = tx.reserve(1)?.start().expect("effect kind");
    tx.insert([&EffectKind {
        id: effect_kind,
        key: "mark-read",
        label: "Mark read",
    }])?;
    Ok((notice, edition, effect_kind))
}

fn seed(db: &Db<RouterLedger>) -> Seed {
    db.write(|tx| {
        let (account, observation) = seed_ingest(tx)?;
        let (notice, edition, effect_kind) = seed_projection(tx, observation)?;
        Ok(Seed {
            account,
            observation,
            notice,
            edition,
            effect_kind,
        })
    })
    .expect("seed write")
    .expect("seed accepted")
    .value
}

#[test]
fn cursor_must_be_the_result_of_an_ingest_batch() {
    let store = TempStore::new("cursor");
    let db = Db::create(store.path(), RouterLedger)
        .expect("create")
        .expect("schema accepted");
    let seed = seed(&db);

    let result = db
        .write(|tx| {
            let old = Cursor {
                account: seed.account,
                position: CursorDigest(digest(5)),
                payload: PayloadDigest(digest(8)),
                observed_at: 11,
            };
            tx.delete([&old])?;
            tx.insert([&Cursor {
                account: seed.account,
                position: CursorDigest(digest(99)),
                payload: PayloadDigest(digest(98)),
                observed_at: 20,
            }])?;
            Ok(())
        })
        .expect("engine");
    assert!(matches!(result, Admission::Rejected(_)));
}

#[test]
fn every_delivery_phase_has_exactly_its_evidence_arm() {
    let store = TempStore::new("delivery-arm");
    let db = Db::create(store.path(), RouterLedger)
        .expect("create")
        .expect("schema accepted");
    let seed = seed(&db);

    let result = db
        .write(|tx| {
            let attempt: DeliveryAttemptId = tx.reserve(1)?.start().expect("attempt");
            tx.insert([&DeliveryAttempt {
                id: attempt,
                edition: seed.edition,
                ordinal: 0,
                phase: DeliveryPhase::Planned.id(),
            }])?;
            Ok(())
        })
        .expect("engine");
    assert!(matches!(result, Admission::Rejected(_)));

    db.write(|tx| {
        let attempt: DeliveryAttemptId = tx.reserve(1)?.start().expect("attempt");
        tx.insert([&DeliveryAttempt {
            id: attempt,
            edition: seed.edition,
            ordinal: 0,
            phase: DeliveryPhase::Planned.id(),
        }])?;
        tx.insert([&DeliveryPlanned { attempt, at: 14 }])?;
        Ok(())
    })
    .expect("engine")
    .expect("planned arm accepted");
}

#[test]
fn effects_can_only_reference_delivered_attempts() {
    let store = TempStore::new("effect-gate");
    let db = Db::create(store.path(), RouterLedger)
        .expect("create")
        .expect("schema accepted");
    let seed = seed(&db);

    let attempt = db
        .write(|tx| {
            let attempt: DeliveryAttemptId = tx.reserve(1)?.start().expect("attempt");
            tx.insert([&DeliveryAttempt {
                id: attempt,
                edition: seed.edition,
                ordinal: 0,
                phase: DeliveryPhase::Planned.id(),
            }])?;
            tx.insert([&DeliveryPlanned { attempt, at: 14 }])?;
            Ok(attempt)
        })
        .expect("engine")
        .expect("attempt accepted")
        .value;

    let rejected = db
        .write(|tx| {
            let intent: EffectIntentId = tx.reserve(1)?.start().expect("intent");
            tx.insert([&EffectIntent {
                id: intent,
                notice: seed.notice,
                delivery: attempt,
                kind: seed.effect_kind,
                phase: EffectPhase::Pending.id(),
                idempotency: EffectDigest(digest(12)),
            }])?;
            tx.insert([&EffectPending { intent, at: 15 }])?;
            Ok(())
        })
        .expect("engine");
    assert!(matches!(rejected, Admission::Rejected(_)));

    db.write(|tx| {
        tx.delete([&DeliveryPlanned { attempt, at: 14 }])?;
        tx.delete([&DeliveryAttempt {
            id: attempt,
            edition: seed.edition,
            ordinal: 0,
            phase: DeliveryPhase::Planned.id(),
        }])?;
        tx.insert([&DeliveryAttempt {
            id: attempt,
            edition: seed.edition,
            ordinal: 0,
            phase: DeliveryPhase::Delivered.id(),
        }])?;
        tx.insert([&DeliveryDelivered { attempt, at: 16 }])?;
        Ok(())
    })
    .expect("engine")
    .expect("delivery transition accepted");

    db.write(|tx| {
        let intent: EffectIntentId = tx.reserve(1)?.start().expect("intent");
        tx.insert([&EffectIntent {
            id: intent,
            notice: seed.notice,
            delivery: attempt,
            kind: seed.effect_kind,
            phase: EffectPhase::Pending.id(),
            idempotency: EffectDigest(digest(12)),
        }])?;
        tx.insert([&EffectPending { intent, at: 17 }])?;
        Ok(())
    })
    .expect("engine")
    .expect("effect after delivery accepted");
}

#[test]
fn nominal_id_spaces_may_share_a_number_without_aliasing() {
    let store = TempStore::new("seed");
    let db = Db::create(store.path(), RouterLedger)
        .expect("create")
        .expect("schema accepted");
    let seed = seed(&db);
    assert_eq!(seed.observation.0, seed.notice.0);
}

#[test]
fn a_reprint_reason_must_match_the_original_terminal_coordinate() {
    let store = TempStore::new("reprint");
    let db = Db::create(store.path(), RouterLedger)
        .expect("create")
        .expect("schema accepted");
    let seed = seed(&db);
    let (original, reprint) = db
        .write(|tx| {
            let original: DeliveryAttemptId = tx.reserve(1)?.start().expect("original");
            tx.insert([&DeliveryAttempt {
                id: original,
                edition: seed.edition,
                ordinal: 0,
                phase: DeliveryPhase::Ambiguous.id(),
            }])?;
            tx.insert([&DeliveryAmbiguous {
                attempt: original,
                at: 14,
                detail: PayloadDigest(digest(20)),
            }])?;
            let reprint: DeliveryAttemptId = tx.reserve(1)?.start().expect("reprint");
            tx.insert([&DeliveryAttempt {
                id: reprint,
                edition: seed.edition,
                ordinal: 1,
                phase: DeliveryPhase::Planned.id(),
            }])?;
            tx.insert([&DeliveryPlanned {
                attempt: reprint,
                at: 15,
            }])?;
            Ok((original, reprint))
        })
        .expect("engine")
        .expect("attempts accepted")
        .value;

    let wrong = db
        .write(|tx| {
            tx.insert([&Reprint {
                attempt: reprint,
                original,
                reason_ref: ReprintReason::AfterFailure.id(),
            }])?;
            Ok(())
        })
        .expect("engine");
    assert!(matches!(wrong, Admission::Rejected(_)));

    db.write(|tx| {
        tx.insert([&Reprint {
            attempt: reprint,
            original,
            reason_ref: ReprintReason::AfterAmbiguity.id(),
        }])?;
        Ok(())
    })
    .expect("engine")
    .expect("matching reprint reason accepted");
}
