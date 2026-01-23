# Implementation Plan - Prompt Kits (Feature 4.3)

**Author**: Antigravity
**Date**: 2026-01-23
**Spec Reference**: `docs/20260123-SPECS-prompt-kits.md`

## 1. `llm` Crate Updates

- [ ] Define `PromptKit` trait in `llm/src/kits.rs` (mod `kits`).
- [ ] Implement `DraftReplyKit` (move existing logic here).
- [ ] Implement `SummarizeKit`.
- [ ] Implement `ActionItemsKit`.
- [ ] Implement `SentimentKit`.
- [ ] Create a `KitRegistry` (or a simple function `get_all_kits()`) to list available options.

## 2. `app` Crate wiring

- [ ] Update `App` state to track `selected_prompt_kit`.
- [ ] In `handle_export_selected`, use the selected kit to format the `LlmRequest`.
  - `request.system_prompt = kit.system_prompt()`
  - `request.user_instruction = kit.format_user_prompt(transcript, user_input)`

## 3. `ui` Crate Updates

- [ ] In the Export/Draft Modal (or wherever the LLM trigger happens), add a cycle/selector for Prompt Kit.
- [ ] Display the current Kit Name/Description in the UI.

## 4. Verification

- [ ] Unit tests in `llm` for prompt formatting.
- [ ] Manual test: Select 'Summarize', verify LLM output is a summary.

## Step-by-Step

1.  Create `llm/src/kits.rs` and the traits/structs.
2.  Expose them in `llm/src/lib.rs`.
3.  Update the calling code in `app` to use a kit (hardcoded `DraftReplyKit` first).
4.  Add UI controls to switch kits.
