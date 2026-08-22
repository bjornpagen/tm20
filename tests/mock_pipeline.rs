use paper_attention_router::providers::gmail::{
    GmailAccount, GmailConnector, GmailCursor, GmailHeader, GmailHistory, GmailHistoryResponse,
    GmailMessage, GmailMessageAdded, GmailMessageRef, GmailPayload, ModifyMessage,
};
use paper_attention_router::{
    Candidate, Connector, CursorCodec, CursorToken, DeliveryMachine, Digest, DigestItem, EditionId,
    EffectOutcome, EffectTarget, EffectVerb, ErasedConnector, ExternalKey, HttpRequest,
    InterruptionWindow, Lane, MockHttp, Privacy, ProviderLocator, Section, SenderTrust, Signal,
    SourceClass, SourceCopy, UnixMillis, Urgency, default_policy, render_digest,
};

fn message(unread: bool) -> GmailMessage {
    let mut label_ids = vec!["INBOX".into()];
    if unread {
        label_ids.push("UNREAD".into());
    }
    GmailMessage {
        id: "18d00a".into(),
        thread_id: "thread-1".into(),
        history_id: "101".into(),
        label_ids,
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

fn gmail_fixture() -> (
    ErasedConnector<GmailConnector<MockHttp>>,
    CursorToken,
    ExternalKey,
) {
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
    .expect("history fixture");
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
        message(true),
    )
    .expect("message fixture");

    let idempotency = ExternalKey([9; 32]);
    http.expect_json(
        HttpRequest::post_json(
            "/gmail/v1/users/me/messages/18d00a/modify",
            ModifyMessage {
                remove_label_ids: vec!["UNREAD".into()],
                add_label_ids: Vec::new(),
            },
        )
        .expect("modify request"),
        200,
        message(false),
    )
    .expect("modify fixture");

    let account = GmailAccount::parse("me@example.com").expect("account");
    let gmail = GmailConnector::new(http, account);
    let cursor = GmailCursor {
        history_id: "100".into(),
    }
    .encode();
    (ErasedConnector::new(gmail), cursor, idempotency)
}

#[test]
fn gmail_mock_runs_through_route_paper_delivery_and_writeback() {
    let (mut connector, cursor, idempotency) = gmail_fixture();
    let batch = connector.pull(Some(&cursor)).expect("pull");
    assert_eq!(batch.observations.len(), 1);
    let observation = &batch.observations[0];

    let candidate = Candidate {
        class: SourceClass::Mail,
        signal: Signal::DirectMessage,
        urgency: Urgency::Normal,
        trust: SenderTrust::Untrusted,
        window: InterruptionWindow::Open,
    };
    let policy = default_policy();
    let decision = policy.route(candidate);
    assert_eq!(decision.lane, Lane::NextDigest);
    assert_eq!(decision.privacy, Privacy::MetadataOnly);

    let copy = SourceCopy::excerpt(
        "Status",
        "The private body must not reach metadata-only paper.",
    )
    .expect("source copy")
    .project(decision.privacy)
    .expect("privacy projection");
    assert_eq!(copy.as_str(), "Status");
    let item =
        DigestItem::parse(Section::Mail, "Gmail", "Ada", "now", copy, 1).expect("digest item");
    let digest = Digest::parse("Now", "D-MOCK", vec![item], 1).expect("digest");
    let rendered = render_digest(&digest);
    assert!(rendered.markdown.contains("Status"));
    assert!(!rendered.markdown.contains("private body"));

    let encoded = ExternalKey([3; 32]);
    let delivered = DeliveryMachine::plan(
        paper_attention_router::AttemptId(1),
        EditionId(1),
        0,
        UnixMillis(1),
    )
    .encoded(UnixMillis(2), encoded)
    .transmitting(UnixMillis(3))
    .delivered(UnixMillis(4));
    let verb = policy
        .effects_for(candidate, decision)
        .next()
        .expect("mail writeback");
    assert_eq!(verb, EffectVerb::MarkRead);
    let effect = DeliveryMachine::release_effect(
        &delivered,
        idempotency,
        verb,
        EffectTarget {
            account: observation.account,
            object: observation.object,
            locator: ProviderLocator::parse("gmail:message:18d00a").expect("locator"),
        },
    );
    assert!(matches!(
        connector.apply(effect.request).expect("mark read"),
        EffectOutcome::Applied { .. }
    ));

    let gmail = connector.into_inner();
    assert_eq!(gmail.into_inner().finish().expect("transcript").len(), 3);
}
