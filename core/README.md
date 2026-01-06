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

- Error log file: `data/logs/app-error.log` (configured in
  `app/config/app.toml` under `[logging].error_log_file`).
- Log level: configured in `app/config/app.toml` under `[logging].level`
  (default `info`).
