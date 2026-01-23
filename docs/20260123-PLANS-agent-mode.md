# Implementation Plan - Agent Mode

## 1. Dependencies

- [ ] Add `clap` to `app/Cargo.toml` with `derive` feature.

## 2. Core Logic (`app/src/agent.rs`)

- [ ] Create `agent.rs` module.
- [ ] Define `AgentScenario` enum (`DraftReply`, etc.).
- [ ] Implement `run_agent_loop`:
  - accepts `CacheManager`, `UiCacheBridge`, `LlmProvider`, etc.
  - initializes event stream.
  - implements the step-by-step logic for `DraftReply`.
  - prints explicit markers: `[AGENT-TEST] STATUS: OK`.

## 3. Entry Point (`app/src/main.rs`)

- [ ] Define `Args` struct using `clap`.
- [ ] Parse `Args` in `async_main`.
- [ ] Branch:
  - If `--agent-test` is present -> call `agent::run_agent_loop`.
  - Else -> call `tui::run_tui_loop`.

## 4. Scenario: Draft Reply

- [ ] Wait for `UiCacheBridge` to populate with dialogs.
- [ ] Pick the first chat.
- [ ] Trigger `UiAction::ExportSelected` handling logic (programmatically).
  - _Note_: We might need to expose `handle_export_selected` pub or move it to a shared controller to avoid code duplication between `tui.rs` and `agent.rs`.
  - _Decision_: Refactor `handle_export_selected` in `tui.rs` to a shared `controller.rs` or `actions.rs` if possible, OR just duplicate the logic wrapper in `agent.rs` calling the shared internal functions.
  - `handle_export_selected` logic is currently in `tui.rs`. It's better to move logic to `actions.rs` in `app` crate.

## 5. Verification

- [ ] Run `cargo run -- --agent-test draft_reply`.
- [ ] Verify exit code and stdout.
