# Agent Native Testability (Agent Mode)

## Context

The Telegram LLM TUI Client aims to be an "Agent Native" application (AND). This means it must be fully testable and operable by an LLM agent without human intervention or visual interpretation (OCR).

Currently, the app only runs as a TUI, which captures stdout/stderr and requires keyboard input, making it hard for an agent to verify functionality autonomously.

## Goals

1. **Headless Operation**: Add a CLI mode that bypasses the TUI completely.
2. **Scenario Execution**: Allow the agent to trigger specific functional flows (e.g., "Draft Reply").
3. **Verifiable Output**: Emit structured or grep-able logs to stdout indicating success or failure.
4. **Graceful Exit**: The app must terminate automatically after the scenario completes.

## CLI Design

Introduce command line arguments:

```bash
telegram-llm-tui --agent-test <SCENARIO>
```

Scenarios:

- `draft_reply`:
  1. Connect to Telegram.
  2. Select a chat with messages.
  3. Select the latest message.
  4. Invoke the "Default Prompt Kit" (Reply).
  5. Wait for the generated draft.
  6. Verify draft is non-empty.
  7. Exit 0 on success, 1 on timeout/error.

## Architecture Changes

### `app` Crate

- **Dep**: Add `clap` for argument parsing.
- **New Module**: `agent.rs` containing the headless runtime.
- **Refactor**: `main.rs` to parse args and branch between `tui::run_tui_loop` and `agent::run_agent_loop`.

### `agent.rs` Logic

The agent loop will act similarly to the TUI loop but driven by a state machine or script rather than user input.

```rust
pub async fn run_agent_loop(
    components: AppComponents,
    scenario: AgentScenario
) -> Result<()> {
    // 1. Establish event stream (same water as TUI)
    // 2. Wait for initial sync
    // 3. Execute Scenario Steps
    // 4. Report Result
}
```

## Security & Safety

- **Mocking**: The agent mode should support using `MockProvider` for LLM to avoid costs during routine verification, but support real providers if configured.
- **State**: It utilizes the real local DB cache, so it is a true integration test.
- **Write Safety**: The `draft_reply` scenario is read-only (it generates a draft but doesn't send it). Future write scenarios (sending messages) must be guarded or strictly labeled.

## Agent Persona Compliance

This feature directly supports the "Droid" persona by providing the "Parity" and "Verification" tools required by the AND principles.
