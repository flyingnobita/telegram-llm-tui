# Project Spec, Requirements & Decision Records

This file defines *where* project decisions are captured and how the spec evolves.

## Decision storage (Early decisions in PLANS.md §0)

We will store concrete decisions as short ADRs (Architecture Decision Records).

- Location: `docs/adr/`
- Naming: `YYYYMMDD-short-title.md`
- When: create an ADR as soon as a decision is made (Telegram lib, TUI lib, LLM
  providers, data-dir layout, etc.)

### ADR template (copy into each new ADR file)

```md
# Title

Date: YYYY-MM-DD
Status: Proposed | Accepted | Superseded

## Context
What problem are we solving? What constraints exist?

## Decision
What did we decide?

## Rationale
Why this over alternatives?

## Consequences
Trade-offs, risks, follow-ups.
```

## Index of Early Decisions (placeholders until decided)

- Telegram client library: grammers → `docs/adr/20251231-telegram-client-grammers.md`
- TUI framework: ratatui → `docs/adr/20251231-tui-ratatui.md`
- LLM providers & auth strategy: OpenAI + local `.env` for dev-only (prod secret
  store later) → `docs/adr/20251231-llm-backend-openai-env.md`
- Data directory layout: local project dir for dev-only → `docs/adr/20251231-data-dir-local.md`
- LLM-friendly test framework: `insta` snapshots + `ratatui` test backend →
  `docs/adr/20260105-llm-friendly-test-framework.md`
- Logging policy (plain logs, 1 MB rotation, 20 files, content logging on) →
  `docs/adr/20260106-logging-policy.md`

## Requirements

### Functional

- Authenticate to Telegram (MTProto via grammers) and sync chats/messages.
- Read chats and send messages (text to start; replies/edits as MVP+).
- Export selected chat context to LLM for analysis and draft responses.
- Present LLM draft to the user for review/edit, then explicit send.
- Operate as a terminal UI with keyboard-first navigation.

### Non‑functional

- Reliable message delivery with retries/backoff.
- Respect Telegram rate limits; never block the UI thread.
- Logs are human-readable plain text; Telegram/LLM content logging is enabled
  by default.
- Console output uses ANSI colors for readability; log files are non-ANSI.
- Log timestamps use local time with RFC 3339 offset.
- Treat logs as sensitive data; do not log auth tokens.
- Works on macOS first; Linux/Windows follow-up.
- Primary log file: `data/logs/app.log` (configured in `app/config/app.toml`
  under
  `[logging].log_file`).
- Error log file: `data/logs/app-error.log` (configured in
  `app/config/app.toml` under `[logging].error_log_file`).
- Log level: configured in `app/config/app.toml` under `[logging].level`
  (default `info`).
- Log rotation: size-based at 1 MB, keep 20 files (configured in
  `app/config/app.toml` under `[logging].rotation_max_size_mb` and
  `[logging].rotation_max_files`).

### Acceptance criteria (MVP)

- User can log in, select a chat, read history, and send a message.
- User can select messages, generate an LLM draft, edit it, and send.
- App recovers from temporary disconnect without losing drafts.

## Spec evolution

- High-level product goals live in `PLANS.md`.
- Detailed behavior, workflows, and constraints live in this SPEC.
- Implementation details and low-level docs should live close to code (e.g.,
  `core/README.md`).
