# Mock provider protocols

No live credentials or network client ship. Every connector is generic over
`HttpTransport`; `MockHttp` verifies exact request/response transcripts.

## Hacker News

Public Firebase API shapes:

- `GET /v0/maxitem.json`
- `GET /v0/topstories.json`
- `GET /v0/item/{id}.json`

Cursor: `{ max_item, ranked[] }`. `maxitem` supports outage recovery; rankings
are a separate, bounded snapshot. Item ID is identity. Score, descendant
count, deletion, and death contribute to revision events.

## Gmail

- `GET /gmail/v1/users/me/profile`
- `GET /gmail/v1/users/me/messages?labelIds=INBOX&maxResults=25`
- `GET /gmail/v1/users/me/history?startHistoryId=…&historyTypes=messageAdded&labelId=INBOX`
- `GET /gmail/v1/users/me/messages/{id}?format=metadata&metadataHeaders=…`
- `POST /gmail/v1/users/me/messages/{id}/modify`
  with `{ "removeLabelIds": ["UNREAD"] }`

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
Socket events wake the connector; history closes gaps. `event_id` deduplicates
delivery while `(team, channel, ts)` identifies a message.

## Google Chat

`GET /chat/v1/{space}/spaceEvents` with:

- `filter=eventTime > "{RFC3339 watermark}"`
- created/updated/deleted message event types
- `pageSize=1000`
- `pageToken` while paginating

Cursor: one overlapped event-time watermark per selected space. Event resource
name identifies delivery; full message and thread resource names identify
objects. No read-state writeback is advertised.

## iMessage bridge protocol v1

This protocol is invented for the separately implemented bridge.

### Discovery

`GET /bridge/v1/info`

```json
{
  "bridge_id": "phone-1",
  "protocol_version": 1,
  "capabilities": [
    "event_backfill",
    "push_events",
    "read_receipts",
    "attachments"
  ]
}
```

Changing `bridge_id` invalidates the cursor instead of silently joining two
message histories.

### Recovery

`GET /bridge/v1/events?after={sequence}&limit=500`

```json
{
  "events": [],
  "next_sequence": 42,
  "has_more": false
}
```

Every event has monotonic `sequence`, stable `event_id`, RFC3339
`occurred_at`, and one tagged variant:

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

`POST /bridge/v1/conversations/{conversation_id}/read-receipts`

```json
{
  "through_message_id": "msg-1",
  "idempotency_key": "64 lowercase hex characters"
}
```

Response:

```json
{
  "receipt_id": "receipt-1",
  "status": "applied",
  "applied_at": "2026-08-22T15:01:00Z"
}
```

`status` is `applied` or `already_applied`. The bridge must return the same
receipt for the same idempotency key.
