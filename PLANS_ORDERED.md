# Telegram LLM TUI Client — Execution Plan (Ordered by Prerequisites)

This file is a prerequisite-ordered duplicate of `PLANS.md`. It preserves the feature scope while making dependencies explicit to guide implementation order.

## [x] 0) Early decisions (do these first)

**Prerequisites:** None.

1. [x] Telegram client: use `grammers` (MTProto) for the MVP. See `docs/adr/20251231-telegram-client-grammers.md`.
2. [x] TUI framework: use `ratatui`. See `docs/adr/20251231-tui-ratatui.md`.
3. [x] LLM backend: start with OpenAI; store API keys in local `.env` for dev-only; implement a proper prod secret store later. See `docs/adr/20251231-llm-backend-openai-env.md`.
4. [x] Data directory: store under the project local folder for dev-only; switch to OS-specific dirs later. See `docs/adr/20251231-data-dir-local.md`.
5. [x] LLM-friendly Test Framework: decide on a testing framework for this project (favoring deterministic, LLM-friendly tests).

## 1) Project scaffold

**Prerequisites:** Early decisions complete.

1. [ ] Create Cargo workspace with crates: `app` (bin, wiring), `core` (Telegram + domain), `ui` (TUI components), `llm` (providers, prompt templates), `integration-tests`.
2. [ ] Add CI basics: `cargo fmt -- --check`, `clippy -D warnings`, `nextest`. Set Rust toolchain in `rust-toolchain.toml`.
3. [ ] Keep Telegram integration grammers-only (MTProto) and document any native deps if they appear.
4. [ ] Use mise-en-place to manage tool versions (Rust toolchain, build deps, CLI helpers).
5. [ ] Set up a TUI test harness (headless render/snapshot) and seed unit tests for UI input/behavior.

## 2) Telegram core

**Prerequisites:** Project scaffold complete; Telegram client decision locked in.

1. [ ] Implement client bootstrap (grammers session config, auth flow, phone/QR login) and update pump (background async task).
2. [ ] Model domain events (new message, edited, read receipt, typing) and expose as channels/streams to the UI layer.
3. [ ] Implement send pipeline with rate-limit/backoff handling; support text, reply, edit, delete; queue unsent messages when offline.
4. [ ] Add minimal persistence for chat metadata and message cache to reduce network round-trips; keep caches small and pluggable.

## 3) TUI experience

**Prerequisites:** Project scaffold and Telegram core (domain events + message data).

1. [ ] Layout v1: left chat list, main message view, bottom composer; modal for LLM-generated drafts; command palette for actions.
2. [ ] Input ergonomics: vim/VSCode-style keymaps, scrollback, search in chat, message selection for LLM export.
3. [ ] Accessibility: color themes (light/dark/high-contrast), configurable keybindings, resize handling, mouse optional.
4. [ ] Notifications: status bar for connection state; optional desktop notifications via feature flag.

## 4) LLM workflow

**Prerequisites:** Project scaffold, Telegram core message data, and TUI selection/draft UI.

1. [ ] Export pipeline: select messages → structured transcript (with authors/timestamps) → send to provider with chosen prompt.
2. [ ] Draft pipeline: receive LLM draft → show diff vs last user draft → allow edit → user explicitly sends.
3. [ ] Prompt kit: summarize thread, propose reply, extract action items, sentiment/priority tagging; keep prompts versioned.
4. [ ] Safety: truncate/zip transcripts to fit token limits; redact secrets before sending; avoid logging secrets; log prompts/responses for reproducibility (opt-in).

## 5) Tooling, testing, and DX

**Prerequisites:** Project scaffold; depends on core/UI/LLM features for meaningful coverage.

1. [ ] Unit tests for domain logic (rate limits, message queue); integration tests with mocked grammers or recorded sessions.
2. [ ] Snapshot tests for UI rendering (ratatui) using `insta` with deterministic data.
3. [ ] Tracing/logging with `tracing` + `tracing-subscriber`; structured logs to file; toggle verbosity at runtime.
4. [ ] Developer commands: `cargo xtask dev-env` to run a local config wizard and start the app.

## 6) Packaging & release

**Prerequisites:** Core + UI + LLM workflows stable; basic testing and tooling in place.

1. [ ] Ship static binaries per target; verify codesign/notarization for macOS.
2. [ ] Provide `.env.example`, a production secret-store helper script (e.g., keychain), and minimal onboarding doc (phone login steps, API ID/Hash link).
3. [ ] Optional: publish Homebrew tap and AUR package once MVP stabilizes.

## 7) Side quests (MCP & tooling)

**Prerequisites:** Core system stable.

1. [ ] Install helpful MCP servers for the lifecycle: repo/code map for navigation, shell/fs runners for scripted experiments, HTTP client for quick API pokes, and benchmark/trace helpers for profiling prompts.
2. [ ] Consider MCP bench/analysis servers to simulate tool-rich flows during LLM prompt testing once the core is stable.

## 8) MVP acceptance criteria (from SPEC)

**Prerequisites:** Telegram core, TUI experience, LLM workflow, and resilience behavior from core/tooling.

1. [ ] User can log in, select a chat, read history, and send a message.
2. [ ] User can select messages, generate an LLM draft, edit it, and send.
3. [ ] App recovers from temporary disconnect without losing drafts.

## 9) Milestones

**Prerequisites:** Completed items from the relevant sections above.

1. [ ] M0 (week 1): spikes conclude; stacks chosen; scaffold + CI green.
2. [ ] M1 (week 2–3): login, chat list, read/send text, basic TUI layout.
3. [ ] M2 (week 4–5): LLM export → draft → user send loop; transcript saving; keymaps.
4. [ ] M3 (week 6): resilience (offline queue), theming, notifications; first binary release (macOS first).
5. [ ] M4 (week 7+): polish, packaging, docs, optional plugins (filters, rules, extra providers).
