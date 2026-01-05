# Start with OpenAI as the first LLM backend

Date: 2025-12-31
Status: Accepted

## Context
We need an initial LLM provider for draft generation and analysis, while keeping the design open for additional providers later.

## Decision
Implement OpenAI as the first LLM backend. Store API keys in a local `.env` file for dev-only convenience, and move to a proper secret store for production.

## Rationale
- Fastest path to a working LLM workflow.
- `.env` is simple during early development.
- We can add other providers behind a trait later.

## Consequences
- `.env` handling must avoid accidental check-in.
- Production will use a proper secret store (e.g., OS keychain or managed secret manager).
- We'll need a migration path for secrets (keychain/secret manager) later.
- LLM provider interface must stay extensible.
