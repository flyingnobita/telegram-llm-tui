# Repository Guidelines

This repository is in the planning phase. Use this guide to keep decisions, tooling, and documentation consistent until the codebase is scaffolded.

## Project Structure & Module Organization

- Planning and requirements live at the root: `PLANS.md` (execution plan) and `SPEC.md` (requirements + ADR policy).
- Architecture decisions are recorded as ADRs in `docs/adr/` with the naming pattern `YYYYMMDD-short-title.md`.
- Planned Rust workspace layout (per `PLANS.md`): `app/` (binary), `core/` (Telegram + domain), `ui/` (TUI), `llm/` (providers/prompts), and `integration-tests/`.

## Current Decisions (MVP)

- Telegram client: `grammers` (MTProto).
- TUI framework: `ratatui`.
- Data directory: project-local for dev-only; OS-specific dirs planned for production.
- LLM auth: local `.env` for dev-only; production secret store planned.

## Planning Workflow

- `PLANS.md` bullets are checklist items; use `- [ ]` and `- [x]` to track completion.
- When any `*.md` file changes, review other docs for consistency and update them as needed; if a related update could be conflicting or unexpected, notify before making that change.

## Build, Test, and Development Commands

Tool versions are managed via mise-en-place.

- `mise install` — install pinned tool versions (Rust toolchain, helpers).
- `cargo build` — build workspace (once scaffolded).
- `cargo test` — run unit tests (once scaffolded).
- `cargo fmt -- --check` and `cargo clippy -D warnings` — formatting and linting (planned in CI).
- `cargo nextest run` — integration test runner (planned).

## Coding Style & Naming Conventions

- Rust formatting: `rustfmt` defaults (4-space indentation, no tabs).
- Naming: crates/modules `snake_case`, types `UpperCamelCase`, functions/vars `snake_case`.
- ADR files: `docs/adr/YYYYMMDD-short-title.md`.

## Testing Guidelines

- Planned split: unit tests inside crates, integration tests in `integration-tests/`.
- UI snapshot tests will likely use `insta` (see `PLANS.md`).
- Keep test data deterministic and avoid live Telegram/LLM calls in CI.

## Commit & Pull Request Guidelines

- No Git history exists yet, so no established commit message convention. Agree on a standard before first commits (e.g., Conventional Commits).
- PRs should include: a short summary, linked issues (if any), and updates to `PLANS.md`, `SPEC.md`, or `docs/adr/*` when decisions change.

## Decision Records (ADRs)

- Create an ADR as soon as a decision is made.
- Reference the relevant ADR in PR descriptions and planning updates.
