# LLM Export Pipeline & UI Polish (Tasks 4.1 & BUGS-2, BUGS-3)

This PR implements the Export Pipeline (Task 4.1) and addresses several UI bugs and discovery improvements.

## 🚀 Features

### LLM Export Pipeline (Task 4.1)
- **Global Export:** Wired `Ctrl+e` to trigger the export of selected messages.
- **Transcript Formatter:** Added logic to convert Telegram messages into a structured text transcript (`[Timestamp] Author: Content`).
- **LlmProvider Integration:** Defined the `LlmProvider` trait in the `llm` crate and implemented a `MockProvider` to echo results back to the composer.
- **Select All in View:** Added `Ctrl+a` functionality in the messages pane to quickly select all currently visible messages.

### Command Palette (Task 3.1)
- **Initial Implementation:** Added a Command Palette UI (`Ctrl+p`).
- **Actions:** Populated the palette with "Export Selected to LLM" as the first available command.

### UI Polish & Notifications
- **Status Messages:** Added a notification area in the bottom bar to show active processing (e.g., "Processing export...").
- **Visual Feedback:** The status message automatically clears once the LLM draft is populated into the composer.

## 🐛 Bug Fixes

- **Fixed Bug #2:** Exposed the Log Window toggle (`Ctrl+l`) in the bottom key hints for better discoverability.
- **Fixed Bug #3:** Added the Quit command (`Ctrl+q`) to the bottom key hints.

## 🛠 Technical Changes

- **Linting:** Resolved all `clippy` warnings (including unused imports and argument complexity).
- **Formatting:** Applied `cargo fmt` project-wide.
- **Testing:** 
    - Updated UI snapshot tests to reflect new bottom-bar key hints.
    - Verified all existing 100+ tests pass.

## 📸 Keybindings Updated
| Key | Action | Pane |
|---|---|---|
| `Ctrl + p` | Open Command Palette | Global |
| `Ctrl + e` | Export Selected to LLM | Global |
| `Ctrl + a` | Select All in View | Messages |
| `Ctrl + l` | Toggle Log Window | Global |
| `Ctrl + q` | Quit | Global |
