# LLM Transcript Logging Specification

## Goal

Create a dedicated log file for LLM conversations that is easy to analyze, starting a new file for each application session.

## Requirements

1. **Log File Location**: `logs/llm/` directory.
2. **Filename Format**:
   - Standard: `YYYY-MM-DD-HH-MM-SS-llm-transcripts.log`
   - Full: `YYYY-MM-DD-HH-MM-SS-llm-transcripts-full.log` (includes full context)
   - Example: `2026-01-24-14-30-00-llm-transcripts-full.log`.
3. **Lifecycle**: Create a new file on every application startup.
4. **Content**:
   - Log User prompts.
   - Log Provider (AI) responses.
   - Clear distinction between User and Provider.
5. **Isolation**: The file should primarily contain the conversation.

## Tech Stack

- Rust
- `tracing`
- `tracing-subscriber`

## Design Attributes

- **Target**: Use a specific tracing target `llm_transcript`.
- **Layering**: Add a specific `tracing-subscriber` layer that filters for this target and writes to the dedicated file.
- **Format**: Plain text, timestamped (implied by `tracing`, but the file itself is timestamped so inside-file timestamps can be minimal or standard).

## Example Output

```text
2026-01-24T14:30:05.123Z INFO User: Hello, how are you?
2026-01-24T14:30:06.456Z INFO Provider: I am an AI, I am doing well.
```
