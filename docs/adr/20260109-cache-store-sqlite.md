# Cache persistence uses sqlite

Date: 2026-01-09
Status: Accepted

## Context

We need minimal persistence for chat metadata and message cache to reduce
network round trips. The store should be durable, bounded, and easy to evolve
without locking us into an in-memory format.

## Decision

Use sqlite as the default cache store backend for chat metadata and message
cache persistence.

## Rationale

Sqlite provides atomic writes, simple schema evolution, and good durability
without adding a separate service. It also supports bounded queries for recent
messages without loading the full cache into memory.

## Consequences

We will add a sqlite schema and migration path in the cache store
implementation. Future alternative backends remain possible via a store trait,
but sqlite will be the default for local dev and MVP.
