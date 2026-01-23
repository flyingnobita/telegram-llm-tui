# SPECS: LM Studio / Local LLM Support

**Date:** 2026-01-23
**Status:** Implemented
**Feature:** 4.5
**Related Plans:** `docs/20260123-PLANS-lm-studio-support.md`

## 1. Background

The application currently relies on a `MockProvider` or a hardcoded OpenAI implementation. Users want to use local LLM runners (like LM Studio or Ollama) to generate drafts without sending data to external external APIs. These local runners typically offer an OpenAI-compatible API but hosted at a custom base URL (e.g., `http://localhost:1234/v1`).

## 2. Goals

1. **Local Privacy**: Allow generation of message drafts without internet access or third-party data processing.
2. **Configurability**: Support configuration of custom Base URLs and Models via `app.toml`.
3. **Extensibility**: Refactor the configuration shape to easily support more providers in the future.

## 3. Detailed Behavior

### 3.1 Configuration (`app.toml`)

- The config must support a `[llm]` section.
- Users must be able to select a `provider` (enum: `lm_studio`, `openai`, `mock`).
- Provider-specific settings must be isolated in subsections (e.g., `[llm.lm_studio]`).
- **Defaults**:
  - Provider: `mock` (or `lm_studio` if the user explicitly enables LLM but sets no provider).
  - Base URL: `http://localhost:1234` (standard LM Studio port).
  - Model: `gpt-3.5-turbo` (standard default alias).

### 3.2 Provider Logic (`OpenAiProvider`)

- The provider struct must accept an optional `base_url`.
- If `base_url` is provided, the client must route requests there.
- If `base_url` is missing, it should default to OpenAI's public API (for the `openai` provider case) or error (for `lm_studio`).
- **Compatibility**: The implementation must handle the slight variations in local server responses if necessary (though most are compliant with `chat/completions` format).
- **Authentication**: Local providers often ignore API keys, but the client should send a dummy key if required by the SDK to prevent errors.

### 3.3 TUI Interaction

- **Export Selected**: When the user triggers "Export to LLM", the system must instantiate the _configured_ provider, not a hardcoded one.
- **Feedback**: The TUI should notify the user "Processing..." while the async request is in flight.
- **Failures**: Network errors (e.g., LM Studio not running) must be caught and displayed as a notification ("Connection refused"), falling back gracefully.

## 4. Constraints

- **Async**: All LLM I/O must be async and non-blocking to the TUI.
- **Dependencies**: Prefer `async-openai` crate for standard-compliance, but ensure we can override the config URL.
- **Security**: Logs must not print full API keys if real OpenAI keys are used later.

## 5. Out of Scope

- Streaming responses (Draft pipeline currently expects a single completion).
- Chat templates (We relying on the `chat/completions` API structure).
