# Use grammers for Telegram client

Date: 2025-12-31
Status: Accepted

## Context

We need a Rust Telegram client library for core messaging features. We want fast
iteration for an MVP and the ability to expand later.

## Decision

Use `grammers` as the Telegram client library for the initial implementation.

## Rationale

- Direct MTProto client in Rust with an active ecosystem.
- Avoids native binary management for the early MVP.
- Good fit for a fast local prototype.

## Consequences

- We must implement more client behaviors ourselves compared to a full client
  library.
- Future migration to another client library remains possible but would require
  refactoring.
