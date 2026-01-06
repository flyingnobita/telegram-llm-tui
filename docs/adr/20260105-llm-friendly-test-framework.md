# LLM-friendly Test Framework

Date: 2026-01-05
Status: Accepted

## Context

We need deterministic, automation-friendly tests for a TUI app so that unit and
UI behavior can be validated without a full interactive terminal session. The
framework should support headless rendering, stable snapshots, and synthetic
input events that can be driven by tools/agents.

## Decision

Adopt `insta` for UI snapshot testing using `ratatui`'s `TestBackend`, and use
standard `cargo test` with explicit synthetic input events for UI
behavior/unit tests.

## Rationale

`insta` makes snapshot diffs easy to review and works well with deterministic
render buffers. `ratatui`'s test backend enables headless rendering without a
real terminal. Keeping tests in `cargo test` avoids extra harness complexity and
remains LLM-friendly for automation.

## Consequences

- Adds snapshot review/update workflow (e.g., `INSTA_UPDATE=always`).
- Requires deterministic test data and stable widget layouts.
- Some end-to-end terminal behaviors will still need manual verification.

## Logging

- Error log file: `data/logs/app-error.log` (configured in
  `app/config/app.toml` under `[logging].error_log_file`).
- Log level: configured in `app/config/app.toml` under `[logging].level`
  (default `info`).
