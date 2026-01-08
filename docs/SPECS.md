# Project Specifications

## Requirements

### Functional

- Authenticate to Telegram (MTProto via grammers) and sync chats/messages.
- Read chats and send messages (text to start; replies/edits as MVP+).
- Export selected chat context to LLM for analysis and draft responses.
- Present LLM draft to the user for review/edit, then explicit send.
- Operate as a terminal UI with keyboard-first navigation.
- Provide message navigation ergonomics: keymaps, scrollback, search, and selection.

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
