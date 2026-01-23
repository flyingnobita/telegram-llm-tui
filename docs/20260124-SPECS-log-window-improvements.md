# Log Window Improvements - Refined Specifications

## Goal Description

Enhance log window navigation to use a cursor-based model where the user moves a highlighted line, with improved visual feedback and selection capabilities.

## User Review Required

- **Change in Behavior**: Navigation keys (`Up`/`Down`) will now move a selection cursor instead of scrolling the viewport directly. Viewport scrolling happens automatically to keep the cursor visible.

## Proposed Changes

### UI Logic (`ui/src/view.rs`, `ui/src/interaction.rs`)

- **Cursor Model**:
  - Maintain a `cursor: Option<usize>` in `LogViewState` representing the active line index.
  - When log window opens, `cursor` defaults to the last line (latest log).
- **Navigation**:
  - `Up`/`Down`/`j`/`k`: Move key -> Move `cursor` index.
  - Clamp cursor to `[0, total_wrapped_lines - 1]`.
  - Auto-scroll viewport to ensure `cursor` is between `scroll` and `scroll + page_size`.
- **Selection**:
  - Selection is now defined as a range `[anchor, cursor]`.
  - **Normal Navigation**: Moves both `anchor` and `cursor` (single line selection/highlight).
  - **Shift + Navigation**: Moves `cursor`, keeps `anchor` fixed (range selection).
- **Rendering**:
  - Highlight line at `cursor`.
  - If `anchor != cursor`, highlight range between them.

### Verification Plan

- **Unit Tests**:
  - Test cursor movement clamps to bounds.
  - Test auto-scrolling when cursor moves out of view.
  - Test shift-selection behavior.
- **Manual Verification**:
  - Open logs, verify last line highlighted.
  - Press `Up` 10 times, verify cursor moves up and view scrolls if needed.
  - Press `Shift+Down`, verify selection expands.
  - Press `C`, verify copied text matches selection.
