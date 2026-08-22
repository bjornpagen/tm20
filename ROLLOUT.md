# Rollout

Rollout is capability-gated, not date-gated. A stage lands only when its exit
proofs pass. Runtime and scheduling remain deferred; these criteria apply to
whichever host eventually owns the library.

## 0. Synthetic foundation

Inputs are fixtures only. No credentials or live effects.

Exit proofs:

- Bumbledb rejects orphan cursors, missing lifecycle arms, pre-delivery
  effects, and mismatched reprint reasons.
- The typed connector erases once into `dyn Connector`; a provider-incompatible
  effect cannot enter the typed implementation.
- Routing is deterministic, total, quiet-hour aware, and never interrupts for
  public feeds.
- Interrupt and digest specimens remain below 800 dots and reject source
  markup as structure.
- A transport interruption after transmission enters `Ambiguous` and cannot
  produce an upstream effect.

## 1. Hacker News

First live adapter because it needs no credentials and has no writeback.

Required representation:

- numeric item identity;
- `maxitem` durable high-water mark;
- separate ranking snapshot for top/best/new;
- revisions update an object while source events remain immutable.

Exit proofs:

- replaying one poll inserts no duplicate observations;
- recovery after a simulated long outage scans unseen item IDs;
- every HN decision is digest or archive, never interrupt;
- empty and overflow digests are bounded.

## 2. Gmail

First private source and first upstream effect.

Required representation:

- account-qualified Gmail message ID and native thread ID;
- mailbox `historyId` cursor;
- bounded full-resync result for an expired history cursor;
- `MarkRead` effect keyed by notice, message, and delivery.

Exit proofs:

- messages and the advanced history cursor commit atomically;
- replay and Pub/Sub duplicate hints are harmless;
- metadata-only printing cannot expose a body;
- only a delivered attempt releases `MarkRead`;
- retrying the same effect returns `AlreadyApplied` or the same receipt without
  reprinting.

## 3. Slack

Socket Mode wakes the adapter; `conversations.history` is the recovery path.

Setup gate: workspace installation, scopes, and membership in selected
conversations.

Exit proofs:

- outer event IDs deduplicate delivery;
- workspace + channel + message timestamp identify a message;
- startup and reconnect reconcile every channel watermark;
- edits and deletions become observations, not replacement notification IDs;
- unselected channels cannot enter routing.

## 4. Google Chat

User OAuth and `spaceEvents.list` per selected space. Pub/Sub may later wake
the same reconciliation path.

Exit proofs:

- full message resource names identify messages;
- event-time cursors use an overlap window plus event-name deduplication;
- subscription expiry loses no events;
- no read-state effect is advertised until the API proves one.

## 5. iMessage bridge

Experimental and last. The bridge stays LAN/VPN-only with pinned TLS.

Setup gate: the exact phone, iOS version, jailbreak, and bridge implementation
pass the protocol-v1 compatibility fixture.

Exit proofs:

- message GUID plus bridge identity is stable across reconnect and backfill;
- REST backfill closes every WebSocket gap;
- attachments, tapbacks, edits, and unsends fail closed when unsupported;
- read receipts are idempotent and are emitted only after delivered paper;
- phone reboot, lock, sleep, and network-change failures are observable and
  recoverable without duplicate paper.

## Explicitly later

No stage above introduces learned priority, fuzzy cross-source threads,
arbitrary replies, attachment printing, cloud sync, multi-user routing, or an
exact-once physical-print claim.
