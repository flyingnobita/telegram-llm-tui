# Logging Policy

Date: 2026-01-06
Status: Accepted

## Context

We need a consistent logging policy for a Telegram + LLM TUI app that supports
troubleshooting and reproducibility while keeping log growth bounded.

## Decision

- Use plain text logs by default.
- Primary log file is `data/logs/app.log`.
- Error log file is `data/logs/app-error.log`.
- Log rotation is size-based at 1 MB per file, keeping 20 files.
- Telegram messages and LLM prompts/responses are logged verbatim by default.
- Default log level is `info`.
- No separate audit log for now.
- Console output uses ANSI colors; log files disable ANSI.
- Timestamps use local time with RFC 3339 offset (e.g., `2026-01-06T16:17:30.358-08:00`).

## Rationale

Plain text logs are easy to read in terminals and log files. Size-based
rotation keeps storage predictable in local development.

## Consequences

- Logs may contain sensitive content. Treat logs as sensitive data and avoid
  logging auth tokens.
- Operators may need to reduce verbosity or disable content logging in
  production if privacy requirements change.
