# Prompt Kits (Feature 4.3)

**Author**: Antigravity
**Date**: 2026-01-23
**Status**: Draft

## 1. Context and Problem

The current LLM export pipeline allows sending a transcript to an LLM, but the prompting strategy is rudimentary. Users perform different tasks on message threads: replying, summarizing, extracting tasks, or analyzing sentiment.

We need a flexible "Prompt Kit" system to:

1.  Encapsulate specific instructions (system prompts) for these different tasks.
2.  Allow the user to select which kit to apply.
3.  Version these prompts to ensure reproducibility and safe iteration.

## 2. Goals

- **Abstraction**: Define a `PromptKit` trait or structure in the `llm` crate.
- **Standard Kits**: Implement the following built-in kits:
  - `Summarize`: Summarize the discussion.
  - `Reply`: Propose a draft reply (the default/current behavior).
  - `ActionItems`: Extract todo list.
  - `Sentiment`: Analyze sentiment/priority.
- **Versioning**: Each kit must have a version identifier.
- **UI Integration**: Allow the user to select the active kit before generation (or defaulting to 'Reply').

## 3. Technical Design

### 3.1 Domain Model (`llm` crate)

We will introduce a `kits` module.

```rust
pub trait PromptKit: Send + Sync {
    /// Unique identifier for the kit (e.g., "summarize")
    fn id(&self) -> &str;

    /// User-friendly display name (e.g., "Summarize Thread")
    fn name(&self) -> &str;

    /// Description for the UI
    fn description(&self) -> &str;

    /// Version of the prompt logic
    fn version(&self) -> &str;

    /// The system prompt to set the behavior/persona
    fn system_prompt(&self) -> String;

    /// How to format the user message.
    /// Receives `transcript` and optional extra `instruction`.
    fn format_user_prompt(&self, transcript: &str, user_instruction: &str) -> String;
}
```

### 3.2 Built-in Kits

1.  **DraftReplyKit** (Default)
    - System: "You are a helpful assistant drafting replies for Telegram..."
    - User: "Context:\n{transcript}\n\nInstruction: {instruction}"
2.  **SummarizeKit**
    - System: "You are an expert summarizer. Condense the following discussion..."
    - User: "Conversation:\n{transcript}"
3.  **ActionItemsKit**
    - System: "Extract all action items, tasks, and deadlines..."
    - User: "Conversation:\n{transcript}"
4.  **SentimentKit**
    - System: "Analyze the tone and priority of this thread..."

### 3.3 UI Changes (`ui` crate)

- In the "LLM Preview" or "Export" modal, add a selector for "Operation" or "Prompt Kit".
- Default to "Draft Reply".
- Pass the selected kit's prompts to the `LlmClient`.

### 3.4 Future Proofing

- File-based prompts (loading from `.prompt` files) is a future enhancement/optimization, not required for MVP. We will hardcode structs for now for type safety and simplicity.

## 4. Test Plan

- **Unit Tests**: Verify each kit generates expected prompt strings.
- **Integration**: (Manual) Verify TUI selector changes the output behavior.

## 5. Alternatives Considered

- **Templates on disk**: Storing prompts in `assets/prompts/*.md`.
  - _Pros_: Easier to edit without recompiling.
  - _Cons_: Harder to package (need to ship assets), parsing overhead.
  - _Decision_: Hardcode in Rust for MVP (Plan 4.3), move to config/file later if needed.
