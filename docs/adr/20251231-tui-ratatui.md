# Use ratatui for the TUI

Date: 2025-12-31
Status: Accepted

## Context

We need a stable, well-supported Rust TUI framework with good examples and
community usage.

## Decision

Use `ratatui` as the TUI framework.

## Rationale

- Mature ecosystem and broad adoption.
- Strong documentation and examples.
- Fits keyboard-first UIs well.

## Consequences

- UI components will follow ratatui's widget model.
- Alternatives (cursive) are deferred for now.

## Logging

- Error log file: `data/logs/app-error.log` (configured in
  `app/config/app.toml` under `[logging].error_log_file`).
- Log level: configured in `app/config/app.toml` under `[logging].level`
  (default `info`).
