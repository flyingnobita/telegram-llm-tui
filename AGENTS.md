# Repository Guidelines

This repository is in the planning phase. Use this guide to keep decisions,
tooling, and documentation consistent until the codebase is scaffolded.

## Agent Mandates

- **Context First:** Before starting any task, read `PLANS.md` to understand
  the current phase and `SPECS.md` for requirements.
- **ADR Adherence:** When making architectural choices, check `docs/adr/`
  first. If a decision contradicts an ADR, stop and ask the user.
- **Plan Updates:** If a task changes the state of a plan (e.g., completing a
  step), you must update `docs/plan-progress/` or `PLANS.md` as part of the
  PR/commit.

## Project Structure & Module Organization

- Planning and requirements live at the root: `PLANS.md` (execution plan) and
  `SPECS.md` (requirements + ADR policy).
- Changelog: `CHANGES.md`
- Architecture decisions are recorded as ADRs in `docs/adr/`.
- Planned Rust workspace layout (per `PLANS.md`):
  - `app/` (binary)
  - `core/` (Telegram + domain)
  - `ui/` (TUI)
  - `llm/` (providers/prompts)
  - `integration-tests/`

## Architecture & Key Decisions

### Current Decisions (MVP)

- Telegram client: `grammers` (MTProto).
- TUI framework: `ratatui`.
- Data directory: project-local for dev-only; OS-specific dirs planned for
  production.
- LLM auth: local `.env` for dev-only; production secret store planned.

### Decision Records (ADRs)

- Create an ADR as soon as a decision is made.
- Reference the relevant ADR in PR descriptions and planning updates.

### Configs

- Any hardcoded values should be placed in the config file, with a short
  description of the value and the values that it can take.
- Config file location: `app/config/app.toml` (Overrides Global Default)

### Logging

- Error log file: `logs/app-error.log` (configured in
  config file under `[logging].error_log_file`).
- Log level: configured in config file under `[logging].level`

## Coding Style & Naming Conventions

- Rust formatting: `rustfmt` defaults (4-space indentation, no tabs).
- Naming: crates/modules `snake_case`, types `UpperCamelCase`, functions/vars
  `snake_case`.

## Development Workflow

### Build, Test, and Development Commands

Tool versions are managed via mise-en-place.

- Install pinned tool versions (Rust toolchain, helpers): `mise install`
- Build workspace: `cargo build`
- Run unit tests: `cargo test`
- Update UI snapshots: `INSTA_UPDATE=always mise exec -- cargo test -p ui`
  during tests.
- Formatting and linting: `cargo fmt -- --check` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Integration test runner (planned) - `cargo nextest run`

### Testing Guidelines

- Planned split: unit tests inside crates, integration tests in `integration-tests/`.
- UI snapshot tests use `insta`.
- Keep test data deterministic and avoid live Telegram/LLM calls in CI.

## Pull Request Guidelines

- In addition to standard commit requirements, PRs must include updates to
  `PLANS.md`, `SPECS.md`, or `docs/adr/*` when decisions change.
