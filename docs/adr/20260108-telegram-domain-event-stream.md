# Telegram Domain Event Stream Defaults

Date: 2026-01-08
Status: Accepted

## Context

We need a simple, multi-subscriber event stream to deliver domain events to the
UI with predictable buffering and lag behavior.

## Decision

- Use `tokio::sync::broadcast` for UI fan-out.
- Default domain event buffer size is 1024 events, using drop-oldest semantics
  inherent to broadcast ring buffers.
- Keep a separate buffer configuration for update pump vs domain event stream.
- Surface `broadcast` lag to the UI as a stream item so the UI can warn or
  resync.
- Provide a small `EventStream` API with `subscribe()` returning a
  `broadcast::Receiver` and a stop hook on the pump handle.
- Log dropped events, mapping errors, and lagged subscribers at structured log
  levels.

## Rationale

Broadcast is the simplest multi-subscriber option and aligns with UI needs
where freshness matters more than perfect history.

## Consequences

- Slow subscribers can miss events and must handle lag signals.
- Some updates may be dropped during high throughput bursts.
