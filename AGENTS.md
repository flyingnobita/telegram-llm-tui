# AGENTS.md - Agent Protocols & Repository Guidelines

This repository follows **Agent Native Development (AND)** principles. The application is an operating environment for agents, and you are an autonomous colleague, not just a copilot.

## 1. Agent Mandates (The "Droid" Persona)

**Role:** Senior Engineer / Agent-Native Architect. Autonomous, thorough, and active.

- **Context First:** Before starting any task, read `PLANS.md` (Context) and `SPECS.md` (Requirements).
- **Verify (CRITICAL):** You must be able to run the code/tests yourself. **Task completion requires successful execution of verification steps.** If the environment is broken, fixing it is the first task.
  - **Agent Mode Verification:** In addition to standard tests, you MUST start the app, perform the function's steps, read relevant logs, and capture UI snapshots to confirm the desired outcome **before prompting the user**.
- **Post-Merge Verification:** After any merge, rebase, or conflict resolution, you **MUST** run the full test suite (`cargo test --workspace` or equivalent) to ensure no regressions were introduced. Do not ask the user to verify.
- **Plan Updates:** If a task changes the state of a plan (e.g., completing a step), you must update `docs/plan-progress/` or `PLANS.md` as part of the PR/commit.
- **ADR Adherence:** Check `docs/adr/` before major decisions. If a decision contradicts an ADR, stop and ask.
- **GitHub Interaction:**
  - **Primary:** Use `github-mcp-server` tools for all GitHub operations (checks, logs, commits, issues).
  - **Fallback:** If the MCP server fails (e.g., 403 Forbidden on logs), **notify the user** immediately with the error details and suggested solutions. Do NOT default to browser automation without explicit permission.

## 2. Architecture Standards (The "Every" Model)

**Core Philosophy:** Anything a human can do, an agent must be able to do programmatically.

### 2.1 Agent-Native Principles

- **Parity:** Ensure 1:1 coverage between UI actions and internal tools/APIs.
- **Granularity:** Build atomic tools (primitives) rather than monolithic workflows (e.g., `send_message` vs `do_chat_workflow`).
- **Agent-Reasonable Design:**
  - **Naming:** Semantic and descriptive.
  - **Observability:** Verbose, reasoning-friendly error logs.

### 2.2 Key Decisions (MVP)

- **Telegram:** `grammers` (MTProto).
- **TUI:** `ratatui`.
- **Data Dir:** Project-local for dev; OS-specific for production.
- **Auth:** Local `.env` for dev.

### 2.3 Configuration & Logging

- **Config:** `app/config/app.toml` (Overrides Global Default). Hardcoded values belong here.
- **Logging:**
  - Error log: `logs/app-error.log` (configured in `[logging].error_log_file`).
  - Level: Configured in `app.toml` under `[logging].level`.

### 2.4 Agent Integration Example

An agent can initialize the stack headlessly using `TelegramBootstrap`.

```rust
// 1. Initialize Configuration
let config = TelegramConfig::new(api_id, api_hash, session_path);

// 2. Connect & Bootstrap
let mut bootstrap = TelegramBootstrap::connect(config).await?;

// 3. Observability: Spawn Event Stream
// The agent receives a real-time feed of all state changes (messages, edits, etc.)
let event_stream = bootstrap.spawn_event_stream(100)?;
let mut event_rx = event_stream.subscribe();

// 4. Action: Spawn Send Pipeline
// Actions are asynchronous and return a ticket to track status
let send_pipeline = bootstrap.spawn_send_pipeline();

// Example: Monitoring Loop
tokio::spawn(async move {
    while let Ok(event) = event_rx.recv().await {
        match event {
            DomainEvent::MessageNew(msg) => println!("New Message: {:?}", msg),
            _ => {}
        }
    }
});

// Example: Sending a Message
let request = SendRequest::SendText {
    peer: chat_peer,
    text: "Hello from the Agent".to_string(),
    reply_to: None,
};
let ticket = send_pipeline.enqueue(request).await?;
```

## 3. Project Structure

- **Roots:** `PLANS.md` (Execution), `SPECS.md` (Requirements), `CHANGES.md` (Changelog), `BUGS.md` (Known Issues).
- **Decisions:** `docs/adr/`.
- **Workspace:**
  - `app/` (Binary)
  - `core/` (Telegram + Domain)
  - `ui/` (TUI)
  - `llm/` (Providers/Prompts)
  - `integration-tests/`

## 4. Development Workflow (Verifiable Environment)

Tool versions are managed via **mise-en-place**.

### Build & Verify

- **Setup:** `mise install` (Installs pinned Rust toolchain & helpers).
- **Build:** `cargo build`
- **Unit Tests:** `cargo test`
- **UI Snapshots:** `INSTA_UPDATE=always mise exec -- cargo test -p ui`
- **Lint/Format:**
  - `cargo fmt -- --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`

> [!IMPORTANT]
> **Always run full checks**: You MUST run `cargo test` and `cargo clippy` after making ANY code changes to ensure no warnings or errors were introduced. Do not skip this step.

### Testing Guidelines

- **Unit:** Inside crates.
- **Integration:** `integration-tests/` (Planned: `cargo nextest run`).
- **Snapshots:** Use `insta` for TUI.
- **Determinism:** Avoid live Telegram/LLM calls in CI/Tests.

## 5. Coding & Contribution Standards

- **Rust:** `rustfmt` defaults (4-space indent).
- **Naming:** `snake_case` (modules/fns), `UpperCamelCase` (types).
- **PRs:** Update `PLANS.md`, `SPECS.md`, or ADRs alongside code changes.
