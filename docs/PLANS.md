# Telegram LLM TUI Client - Execution Plan (Ordered by Prerequisites)

This file is a prerequisite-ordered duplicate of `PLANS.md`. It preserves the
feature scope while making dependencies explicit to guide implementation order,
including sub-feature ordering where applicable.

## [x] 0 Early decisions (do these first)

**Prerequisites:** None.

1. [x] (0.1) Telegram client: use `grammers` (MTProto) for the MVP.
   See `docs/adr/20251231-telegram-client-grammers.md`.
2. [x] (0.2) TUI framework: use `ratatui`.
   See `docs/adr/20251231-tui-ratatui.md`.
3. [x] (0.3) LLM backend: start with OpenAI; store API keys in local `.env`
   for dev-only; implement a proper prod secret store later.
   See `docs/adr/20251231-llm-backend-openai-env.md`.
4. [x] (0.4) Data directory: store under the project local folder for
   dev-only; switch to OS-specific dirs later.
   See `docs/adr/20251231-data-dir-local.md`.
5. [x] (0.5) LLM-friendly Test Framework: decide on a testing framework
   for this project (favoring deterministic, LLM-friendly tests).
6. [x] (0.6) Logging policy: plain logs, size-based rotation, and content
   logging defaults. See `docs/adr/20260106-logging-policy.md`.

## 1 Project scaffold

**Prerequisites:** Early decisions complete. **Sub-feature ordering:** Workspace
-> tooling/version management -> CI/toolchain -> integration constraints -> TUI
test harness.

1. [x] (1.1) Create Cargo workspace with crates: `app` (bin, wiring), `core`
   (Telegram + domain), `ui` (TUI components), `llm` (providers, prompt
   templates), `integration-tests`.
2. [x] (1.2) Use mise-en-place to manage tool versions (Rust toolchain, build
   deps, CLI helpers).
3. [x] (1.3) Add CI basics: `cargo fmt -- --check`, `clippy -D warnings`,
   `nextest`. Set Rust toolchain in `rust-toolchain.toml`.
4. [x] (1.4) Keep Telegram integration grammers-only (MTProto) and document
   any native deps if they appear.
5. [x] (1.5) Set up a TUI test harness (headless render/snapshot) and seed
   unit tests for UI input/behavior.

## 2 Telegram core

**Prerequisites:** Project scaffold complete; Telegram client decision locked in.
**Sub-feature ordering:** Bootstrap -> domain events -> send pipeline ->
persistence.

1. [x] (2.1) Implement client bootstrap (grammers session config, auth flow,
   phone/QR login) and update pump (background async task).
2. [x] (2.2) Model domain events (new message, edited, read receipt, typing)
   and expose as channels or streams to the UI layer.
3. [x] (2.3) Implement send pipeline with rate-limit or backoff handling;
   support text, reply, edit, delete; queue unsent messages when offline.
4. [x] (2.4) Add minimal persistence for chat metadata and message cache to
   reduce network round-trips; keep caches small and pluggable.

## 3 TUI experience

**Prerequisites:** Project scaffold and Telegram core (domain events + message
data). **Sub-feature ordering:** Layout -> input ergonomics -> accessibility ->
notifications.

1. [x] (3.1) Layout v1: left chat list, main message view, bottom composer;
   modal for LLM-generated drafts; command palette for actions.
2. [x] (3.2) Input ergonomics: vim or VSCode-style keymaps, scrollback,
   search in chat, message selection for LLM export.
3. [ ] (3.3) Accessibility: color themes (light/dark/high-contrast),
   configurable keybindings, resize handling, mouse optional.
4. [ ] (3.4) Notifications: status bar for connection state; optional desktop
   notifications via feature flag.

## 4 LLM workflow

**Prerequisites:** Project scaffold, Telegram core message data, and TUI
selection or draft UI. **Sub-feature ordering:** Export pipeline -> draft
pipeline -> prompt kit -> safety.

1. [ ] (4.1) Export pipeline: select messages -> structured transcript (with
   authors or timestamps) -> send to provider with chosen prompt.
2. [ ] (4.2) Draft pipeline: receive LLM draft -> show diff vs last user
   draft -> allow edit -> user explicitly sends.
3. [ ] (4.3) Prompt kit: summarize thread, propose reply, extract action
   items, sentiment or priority tagging; keep prompts versioned.
4. [ ] (4.4) Safety: truncate or zip transcripts to fit token limits; redact
   secrets before sending; avoid logging auth tokens; log prompts or responses
   for reproducibility (default on).

## 5 Tooling, testing, and DX

**Prerequisites:** Project scaffold; depends on core, UI, LLM features for
meaningful coverage. **Sub-feature ordering:** Domain tests -> UI snapshot tests
-> tracing or logging -> dev-env command.

1. [ ] (5.1) Unit tests for domain logic (rate limits, message queue);
   integration tests with mocked grammers or recorded sessions.
2. [ ] (5.2) Snapshot tests for UI rendering (ratatui) using `insta` with
   deterministic data.
3. [ ] (5.3) Tracing or logging with `tracing` + `tracing-subscriber`;
   human-readable logs to `data/logs/app.log`; size-based rotation at 1 MB with
   20 files; toggle verbosity at runtime.
4. [ ] (5.4) Developer commands: `cargo xtask dev-env` to run a local config
   wizard and start the app.

## 6 Packaging and release

**Prerequisites:** Core + UI + LLM workflows stable; basic testing and tooling
in place. **Sub-feature ordering:** Binaries or codesign -> onboarding, docs,
secrets helper -> package managers.

1. [ ] (6.1) Ship static binaries per target; verify codesign or notarization
   for macOS.
2. [ ] (6.2) Provide `.env.example`, a production secret-store helper script
   (e.g., keychain), and minimal onboarding doc (phone login steps, API ID/Hash
   link).
3. [ ] (6.3) Optional: publish Homebrew tap and AUR package once MVP
   stabilizes.

## 7 Side quests (MCP and tooling)

**Prerequisites:** Core system stable. **Sub-feature ordering:** MCP server
setup -> bench or analysis servers.

1. [ ] (7.1) Install helpful MCP servers for the lifecycle: repo or code map
   for navigation, shell or fs runners for scripted experiments, HTTP client
   for quick API pokes, and benchmark or trace helpers for profiling prompts.
2. [ ] (7.2) Consider MCP bench or analysis servers to simulate tool-rich
   flows during LLM prompt testing once the core is stable.

## 8 MVP acceptance criteria (from SPEC)

**Prerequisites:** Telegram core, TUI experience, LLM workflow, and resilience
behavior from core or tooling. **Sub-feature ordering:** Login/chat/send -> LLM
draft loop -> disconnect recovery.

1. [ ] (8.1) User can log in, select a chat, read history, and send a message.
2. [ ] (8.2) User can select messages, generate an LLM draft, edit it, and
   send.
3. [ ] (8.3) App recovers from temporary disconnect without losing drafts.

## 9 Milestones

**Prerequisites:** Completed items from the relevant sections above.
**Sub-feature ordering:** Milestones reflect completion of the ordered sections
above.

1. [ ] (9.1) M0 (week 1): spikes conclude; stacks chosen; scaffold + CI green.
2. [ ] (9.2) M1 (week 2-3): login, chat list, read/send text, basic TUI
   layout.
3. [ ] (9.3) M2 (week 4-5): LLM export -> draft -> user send loop; transcript
   saving; keymaps.
4. [ ] (9.4) M3 (week 6): resilience (offline queue), theming,
   notifications; first binary release (macOS first).
5. [ ] (9.5) M4 (week 7+): polish, packaging, docs, optional plugins
   (filters, rules, extra providers).
