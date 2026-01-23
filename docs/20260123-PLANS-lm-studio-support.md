# PLANS: LM Studio / Local LLM Support

**Date:** 2026-01-23
**Status:** Implemented
**Spec:** `docs/20260123-SPECS-lm-studio-support.md`

## 1. High-Level Goals

1. **Refactor Config**: Split LLM configuration into provider-specific sections `[llm.lm_studio]`, `[llm.openai]`.
2. **Generic Provider**: Implement a flexible `OpenAiProvider` that can target any compatible endpoint.
3. **Dynamic wiring**: Update `main.rs` to select and build the generic provider at runtime.

## 2. Dependencies

- `async-openai`: Standard client for OpenAI-compatible APIs.
- `app.toml`: Needs migration to new schema.

## 3. Implementation Steps

1. **Dependency Update**: Add `async-openai` to `llm/Cargo.toml`.
2. **Config Refactor**:
    - Add `LmStudioConfig` struct.
    - Update `AppConfig` parser.
3. **Provider Implementation**:
    - Update `OpenAiProvider` to builder pattern accepting `base_url` and `api_key`.
    - Handle `None` keys (dummy default).
4. **Application Wiring**:
    - Switch-case in `main.rs` to initialize `OpenAiProvider` with either Local or Remote settings.
5. **Clean Up**: Remove ad-hoc config passing in `UiCacheBridge`.

## 4. Verification

- **Manual Test**: Connect to running LM Studio instance.
- **Unit Test**: `cargo test` ensures config parsing doesn't regress.
