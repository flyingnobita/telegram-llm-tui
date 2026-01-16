# Project Specifications

## Requirements

### Functional

- Authenticate to Telegram (MTProto via grammers) and sync chat summaries plus
  recent message history on startup (default 100 per chat, configurable).
- Read chats and send messages (text to start; replies/edits as MVP+).
- Render service messages with placeholder text in history views.
- Export selected chat context to LLM for analysis and draft responses.
- Present LLM draft to the user for review/edit, then explicit send.
- Operate as a terminal UI with keyboard-first navigation.
- Provide message navigation ergonomics: keymaps, scrollback, search, and selection.
- Provide a log window overlay toggled by a hotkey (default `l`) that is centered,
  90 percent of the main window size, scrollable, and closed with Escape. The
  log window shows the primary log file (`[logging].log_file`). Logs are not
  shown in the main window, and it shows up to `[ui].log_window_max_lines`
  lines.
- Render message authors using display names (name or @username) when available.
- Provide message pane horizontal scrolling with visible scrollbars.
- Provide chat list pane vertical and horizontal scrollbars.
- Use a reusable scrollable pane component for TUI panes (chat list, message
  list, message composer, log window) that:
  - supports vertical scrolling, optional horizontal scrolling, and optional
    scrollbars per axis,
  - clamps scroll offsets to computed max scroll values based on content and
    viewport size,
  - exposes viewport sizing and page size helpers used by the UI state.

### Non‑functional

- Reliable message delivery with retries/backoff.
- Respect Telegram rate limits; never block the UI thread.
- Logs are human-readable plain text; Telegram/LLM content logging is enabled
  by default.
- Console output uses ANSI colors for readability; log files are non-ANSI. When
  the TUI is active, console logging is suppressed to avoid corrupting the UI.
- Log timestamps use local time with RFC 3339 offset.
- Treat logs as sensitive data; do not log auth tokens.
- Works on macOS first; Linux/Windows follow-up.
- Primary log file: `logs/app.log` (configured in `app/config/app.toml`
  under
  `[logging].log_file`).
- Error log file: `logs/app-error.log` (configured in
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
