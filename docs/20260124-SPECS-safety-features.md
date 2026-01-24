# SPECS: 4.4 - Safety Features (Truncation and Redaction)

**Date**: 2026-01-24
**Status**: Implemented (Retroactive Spec)

## Goal

Ensure LLM interactions are safe and robust by:

1. **Truncating** input contexts that exceed token limits (to prevent provider errors).
2. **Redacting** sensitive information (API keys, private keys) from transcripts before they leave the app.

## Requirements

### 1. Input Truncation

- **Constraint**: LLM providers have context windows. We must not send more tokens than allowed.
- **Approximation**: Since `tiktoken` is heavy, use a simple character-based heuristic (e.g., 4 chars = 1 token) for the MVP.
- **Behavior**:
  - If `token_count(transcript) > max_input_tokens`, truncate the transcript string.
  - Append `[TRUNCATED]` to indicate data loss.
- **Configuration**:
  - `llm.max_input_tokens` in `config.toml` (default: 16,384).

### 2. Secret Redaction

- **Scope**: Prevent accidental leakage of keys found in chat history.
- **Targets**:
  - OpenAI-style keys (`sk-...`).
  - Private key blocks (`-----BEGIN PRIVATE KEY...`).
- **Behavior**:
  - Replace detected secrets with `[REDACTED_SECRET]`.
  - Apply this to the **transcript** and the **user input** before formatting the final prompt.
  - Apply _before_ logging (if transcripts are logged).

### 3. User Experience

- **Transparency**: User is notified if export is processed.
- **Loss Indication**: If truncated, the LLM receives the `[TRUNCATED]` marker, so it knows context is partial.

## Implementation Details

- **Location**: `app/src/actions.rs` is the gateway for exports. Safety logic resides there to cover all kits.
- **Shared Logic**: `llm` crate exposes `redact_secrets` and `Tokenizer` for reuse.
