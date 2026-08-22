# Mock provider protocols

No live credentials or network client ship. Every connector is generic over
`HttpTransport`; `MockHttp` verifies exact request/response transcripts.

## Hacker News

Public Firebase API shapes:

- `GET /v0/maxitem.json`
- `GET /v0/topstories.json`
- `GET /v0/item/{id}.json`

Cursor: `{ max_item, ranked[], pending_null[] }`. `maxitem` supports outage
recovery; rankings are a separate, bounded snapshot. An item endpoint may
temporarily return JSON `null`, so its ID remains in `pending_null` until it
materializes. Item ID is identity. Score, descendant count, deletion, and
death contribute to revision events.

## Gmail

- `GET /gmail/v1/users/me/profile`
- `GET /gmail/v1/users/me/messages?labelIds=INBOX&maxResults=25`
- `GET /gmail/v1/users/me/history?startHistoryId=…&maxResults=500`
  with repeated `historyTypes` for message added/deleted and label
  added/removed
- `GET /gmail/v1/users/me/messages/{id}?format=METADATA&metadataHeaders=…`
- `POST /gmail/v1/users/me/messages/{id}/modify`
  with `{ "removeLabelIds": ["UNREAD"], "addLabelIds": [] }`

Cursor: mailbox `historyId`. Message ID and thread ID remain distinct. An
expired history cursor is represented as a provider failure requiring bounded
initial synchronization. Mark-read locators are
`gmail:message:{message_id}` and are idempotent by effect digest.

## Slack

Socket Mode envelope:

```json
{
  "envelope_id": "env-1",
  "payload": {
    "event_id": "Ev-1",
    "team_id": "T1",
    "event": {
      "type": "message",
      "channel": "C1",
      "user": "U1",
      "text": "hello",
      "ts": "1700000000.000100",
      "thread_ts": "",
      "subtype": ""
    }
  }
}
```

Recovery:

`GET /api/conversations.history?channel=…&oldest=…&inclusive=false&limit=200`

Cursor: one `(channel, timestamp)` watermark per selected conversation.
Socket events wake the connector; history closes posted-message gaps.
Posted, changed, and deleted events are distinct wire variants; edits retain
the original message timestamp while using `edited.ts` as revision identity.
`event_id` deduplicates transport delivery while `(team, channel, ts)`
identifies a message. Slack history does not guarantee recovery of missed
edits or deletions; the scaffold exposes that limitation rather than claiming
exact reconciliation.

## Google Chat

`GET /chat/v1/{space}/spaceEvents` with:

- bounded `startTime`/`endTime` filter with a 60-second overlap
- created/updated/deleted message event types
- `pageSize=100`
- `pageToken` while paginating

Cursor: the completed `endTime` per selected space. Single and batch payloads
flatten to one observation per message, keyed by event and message resource
names. Full message and thread resource names identify objects. No read-state
writeback is advertised.

## iMessage bridge protocol v1

This protocol is invented for the separately implemented bridge.

### Discovery

`GET /v1/bridge`

```json
{
  "bridge_id": "phone-1",
  "store_id": "store-1",
  "protocol_version": 1,
  "event_head": "cursor-42",
  "capabilities": [
    "event_backfill",
    "push_events",
    "read_receipts",
    "attachments"
  ]
}
```

Changing either `bridge_id` or `store_id` invalidates the cursor instead of
silently joining two message-store generations.

### Recovery

`GET /v1/events?after={opaque_cursor}&limit=500`

```json
{
  "events": [],
  "next_cursor": "cursor-42",
  "has_more": false
}
```

Every event has an opaque ordered `cursor`, stable `event_id`, signed
`occurred_at_ms`, and one tagged variant:

- `message_created`
- `message_edited`
- `message_unsent`
- `tapback_added`
- `tapback_removed`
- `attachment_available`
- `read_receipt_observed`

Push uses the identical event envelope. It is only a wake-up hint; REST
backfill is authoritative.

### Read receipt

`PUT /v1/read-receipts/{64-lowercase-hex-idempotency-key}`

```json
{
  "conversation_id": "chat-1",
  "through_message_id": "msg-1"
}
```

Response:

```json
{
  "receipt_id": "receipt-1",
  "status": "applied",
  "applied_at_ms": 1776527260000
}
```

`status` is `applied` or `already_applied`. The bridge must return the same
receipt for the same idempotency key.
