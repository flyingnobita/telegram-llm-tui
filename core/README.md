# Core Crate: Telegram & Domain Logic

This crate implements the core domain logic, Telegram client integration (`grammers`), and state management. It is designed as a standalone library that can be used by the TUI `app`, a CLI, or an autonomous Agent.

## Agent Integration (The "Every" Model)

This crate supports **Agent Native Development (AND)**. For usage examples and architectural standards, see `AGENTS.md`.

## Architecture

- **Granularity:** All actions are atomic (e.g., `SendText` vs `EditText`).
- **Observability:** `DomainEvent` provides a comprehensive view of state changes.
- **Parity:** The TUI uses these exact same primitives, ensuring feature parity.

## Dependencies

- **Telegram:** `grammers` (MTProto).
- **Storage:** `sqlite3` (via `grammers-session`).
    - *Note:* Requires native build tools (Xcode on macOS, build-essential on Linux).

## Logging

Logging is configured by the consumer (e.g., `app`).
- **Standard:** `tracing` crate.
- **Levels:** Configurable (INFO/DEBUG/TRACE).
- **Files:** Typically `logs/app.log` and `logs/app-error.log`.