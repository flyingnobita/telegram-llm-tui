# Implement LLM Transcript Logging

## Goal Description

Enable dedicated logging for LLM conversations to `logs/llm/<timestamp>-llm-transcripts.log`.

## User Review Required

- None.

## Proposed Changes

### App

#### [MODIFY] [main.rs](file:///home/omarchy/Data/Projects/Personal/telegram-llm-tui-4-5-add-lm-studio-worktree/app/src/main.rs)

- Modify `init_tracing`:
  - Generate timestamped filename: `logs/YYYY-MM-DD-HH-MM-SS-llm-transcripts.log`.
  - Create a file writer for this path.
  - Add a new `tracing-subscriber` layer:
    - Filter: `target == "llm_transcript"`.
    - Writer: The new file writer.
    - Format: Plain text.

### LLM

#### [MODIFY] [openai.rs](file:///home/omarchy/Data/Projects/Personal/telegram-llm-tui-4-5-add-lm-studio-worktree/llm/src/openai.rs)

- Import `tracing::info`.
- In `generate_draft`:
  - Log prompt: `info!(target: "llm_transcript", "User: {}", request.user_prompt);`.
  - Log response: `info!(target: "llm_transcript", "Provider: {}", text);`.

## Verification Plan

### Automated Tests

- Extend `agent` scenario? (Out of scope for this simple task, manual verification preferred).

### Manual Verification

1. Start the app.
2. Check `logs/llm/` for the new files (standard and full).
3. Perform an LLM request (or mock).
4. Check the file content for User and Provider lines.
5. Restart app and check for a _new_ file.
