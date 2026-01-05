# Core crate

This crate owns the Telegram client and domain logic.

## Telegram stack

- Use `grammers` (MTProto) only for Telegram integration in this crate.
- Do not add alternative Telegram SDKs or HTTP APIs here.

## Native dependencies

None as of 2026-01-05.

If a future change adds native dependencies (for example: OpenSSL, libsodium,
sqlite),
record them here with:

- why the dependency is needed
- which crate introduced it
- install steps per OS (macOS/Linux/Windows)

## Testing

- UI snapshot tests (workspace): `INSTA_UPDATE=always mise exec -- cargo test -p
  ui`
