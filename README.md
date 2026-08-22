# Paper Attention Router

A local-first attention control plane built on
[Bumbledb](https://github.com/bjornpagen/bumbledb) and projected onto paper
through [tm20](https://github.com/bjornpagen/tm20).

This repository is deliberately a library, not a daemon. Runtime, scheduling,
credentials, and live provider clients remain host decisions. The library
defines the stable part first:

- a constraint-checked routing ledger;
- typed provider plugins erased once to `dyn Connector`;
- deterministic routing and writeback policy as data;
- bounded interrupt and digest tapes;
- delivery typestates that distinguish failure from uncertainty;
- capability and rollout data for HN, Gmail, Slack, Google Chat, and SMServer.

## Doctrine

The representation determines the control flow.

- A provider event is an `Observation`, not yet a notification.
- A `Notice` is an explicit routing decision.
- An `Edition` is immutable ordered membership, not a live inbox query.
- `Delivered`, `Failed`, and `Ambiguous` are different types and ledger arms.
- Printing, human reading, and upstream read state are separate facts.
- A post-print `MarkRead` or `SendReadReceipt` is an independently retryable
  `EffectIntent`.
- Push is a wake-up hint. Every plugin owns a durable reconciliation cursor.
- Raw MIME, attachments, and provider JSON live outside Bumbledb behind
  content digests.

## Architecture

```text
typed provider plugin
        │
        ▼
ErasedConnector<T> ──► Box<dyn Connector>
        │
        ▼
NormalizedObservation ──► RouterLedger
        │
        ▼
PolicyTable ──► Notice ──► bounded Edition
        │                       │
        │                       ▼
        │                 tm20 compile
        │                       │
        └──────────────► delivery typestate
                                │
                         Delivered only
                                │
                                ▼
                         UpstreamEffect
```

Provider-native cursor and effect types never cross the typed plugin boundary.
`ErasedConnector<T>` performs the single deliberate erasure needed for a
runtime registry. The policy table is parsed once; its returned type proves
that there is one total fallback, unique precedence, no public-feed interrupt,
and no unsafe full-excerpt rule.

## Modules

- [`schema`](src/schema.rs): Bumbledb theory and lifecycle constraints.
- [`connector`](src/connector.rs): typed plugin API and `dyn` erasure.
- [`policy`](src/policy.rs): routing/effect data and evaluator.
- [`paper`](src/paper.rs): parsed text, hard tape budget, tm20 compilation.
- [`delivery`](src/delivery.rs): compile-time delivery transitions.
- [`rollout`](src/rollout.rs): connector capabilities, setup gates, and order.

## Current connector plan

| Source | Reliable cursor | Wake-up path | Effects |
| --- | --- | --- | --- |
| Hacker News | `maxitem` + ranking snapshot | polling | none |
| Gmail | `historyId` | polling, later Pub/Sub | mark read |
| Slack | channel message timestamp | Socket Mode | none |
| Google Chat | overlapped space-event time | later Pub/Sub | none |
| iMessage | bridge + message GUID | SMServer WebSocket | read receipt |

The sequence is synthetic fixtures, Hacker News, Gmail, Slack, Google Chat,
then the version-fragile jailbroken-phone bridge.

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The tests exercise schema rejections, typed plugin erasure, policy safety,
paper overflow, markup neutralization, typestate delivery, and rollout order.
