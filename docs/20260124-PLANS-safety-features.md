# PLANS: 4.4 - Safety Features

**Date**: 2026-01-24
**Target**: Item 4.4 of PLANS.md

## High Level Goal

Implement truncation and redaction to protect user data and ensure reliable LLM requests.

## User Review Required

- **Token Counting**: Using character-based approximation (4 chars/token) to avoid heavy dependencies.
- **Redaction**: Regex-based "best effort" redaction for standard keys.

## Architecture

### `llm` Crate

- **Helper**: `redact_secrets(text: &str) -> String`.
- **Trait**: `Tokenizer` with `CharTokenizer` implementation.
- **Dependencies**: Add `regex`.

### `app` Crate

- **Config**: Add `max_input_tokens` to `LlmConfig`.
- **Pipeline**: In `handle_export_selected` and `handle_llm_submit`:
  1. Format transcript.
  2. Check length -> Truncate if needed.
  3. Redact secrets.
  4. Pass to `PromptKit`.

## Verification Plan

### Automated Tests

- Unit tests in `llm` for regex patterns and tokenizer math.

### Manual Verification

- **Truncation**: Force low limit in config, export large chat.
- **Redaction**: Paste dummy API key in chat, export, check logs or mock response.
