# Core crate

This crate owns the Telegram client and domain logic.

## Telegram stack

- Use `grammers` (MTProto) only for Telegram integration in this crate.
- Do not add alternative Telegram SDKs or HTTP APIs here.

## Native dependencies

- sqlite3 (via `grammers-session` `SqliteSession` and `sqlite3-src`)
  - why: persist Telegram session/auth keys
  - introduced by: `core` Telegram bootstrap
  - install: macOS needs Xcode Command Line Tools; Linux needs build-essential;
    Windows needs MSVC build tools

If a future change adds native dependencies (for example: OpenSSL, libsodium,
sqlite),
record them here with:

- why the dependency is needed
- which crate introduced it
- install steps per OS (macOS/Linux/Windows)

## Testing

- UI snapshot tests (workspace): `INSTA_UPDATE=always mise exec -- cargo test -p
  ui`

## Logging

- Primary log file: `data/logs/app.log` (configured in `app/config/app.toml`
  under
  `[logging].log_file`).
- Error log file: `data/logs/app-error.log` (configured in
  `app/config/app.toml` under `[logging].error_log_file`).
- Log level: configured in `app/config/app.toml` under `[logging].level`
  (default `info`).
- Log format: plain text (configured in `app/config/app.toml` under
  `[logging].format`).
- Console output uses ANSI colors; log files disable ANSI formatting.
- Log timestamps use local time with RFC 3339 offset.
- Log rotation: size-based at 1 MB, keep 20 files (configured in
  `app/config/app.toml` under `[logging].rotation_max_size_mb` and
  `[logging].rotation_max_files`).
- Content logging: Telegram and LLM content logging enabled by default
  (configured in `app/config/app.toml` under `[logging].log_content`).
