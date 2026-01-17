# Represent message authors as user or chat peers

Date: 2026-01-17
Status: Accepted

## Context

Message authors are currently modeled as user IDs only. Telegram updates can omit
`from_id` or reference non-user peers for channel posts and anonymous admin
messages. The existing mapper drops these updates, which hides valid incoming
messages in channels and groups.

## Decision

Introduce an author identity type that can represent either a user or a chat
peer. Domain events and cached messages will store this author identity and an
optional author display name when provided by Telegram (for example
`post_author`).

## Rationale

This keeps the domain model aligned with Telegram semantics while preventing
message drops. It also preserves author display names for non-user senders
without overloading user identity or forcing incorrect IDs.

## Consequences

- Message and cache types change to include the new author identity and optional
  author name.
- Persistence schema adds author kind and optional author name for messages.
- UI author label resolution must handle user and chat peer authors.
