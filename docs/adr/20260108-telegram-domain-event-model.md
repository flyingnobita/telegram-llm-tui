# Telegram Domain Event Model Defaults

Date: 2026-01-08
Status: Accepted

## Context

We need a stable domain event model to decouple the UI from grammers update
structures while keeping the MVP scope small and adaptable.

## Decision

- Use server-provided IDs wrapped in newtypes: `ChatId`, `MessageId`,
  `UserId`.
- Do not compose peer type into IDs for now.
- Define four domain event types: new message, message edited, read receipt,
  typing.
- Use a minimal payload for each event:
  - New message: `chat_id`, `message_id`, `sender_id` (optional), `timestamp`,
    `timestamp_source`, `text` (optional), `is_outgoing`.
  - Message edited: `chat_id`, `message_id`, `editor_id` (optional),
    `timestamp`, `timestamp_source`, `new_text` (optional).
  - Read receipt: `chat_id`, `message_id`, `reader_id` (optional),
    `timestamp`, `timestamp_source`.
  - Typing: `chat_id`, `user_id`, `is_typing`, `timestamp`,
    `timestamp_source`.
- Timestamp source uses server-provided timestamps when present. When a server
  timestamp is missing, fall back to local receive time and mark
  `timestamp_source = Receive`.
- Map only the four event types above. Drop all other update types with a
  structured log reason.
- Defer channel and group semantics for read receipts and typing events.

## Rationale

These defaults keep the model small, predictable, and easy to extend when
additional Telegram update variants are needed.

## Consequences

- Some update types are intentionally dropped until expanded.
- IDs are simple but may need future adjustment if peer scoping becomes
  necessary.
- Consumers may see receive time timestamps for events lacking server time.
