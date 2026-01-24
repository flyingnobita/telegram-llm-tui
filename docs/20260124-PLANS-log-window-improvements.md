# Implementation Plan - Cursor-Based Log Navigation

## Goal Description

Refactor Log Window to use cursor-based navigation. Instead of scrolling the viewport directly, keys move a highlighted cursor line, and the view follows the cursor.

## User Review Required

> [!NOTE]
> This changes the fundamental interaction model of the log window from a scrollable pane to a navigable list.

## Proposed Changes

### UI Logic (`ui/src`)

#### [MODIFY] [view.rs](file:///home/omarchy/Data/Projects/Personal/telegram-llm-tui-bugfix-log-window-wrapping-worktree/ui/src/view.rs)

- Update `LogViewState`:
  - Rename `selection` to something that clarifies its dual purpose or ensure `cursor` is always present.
  - Actually, `selection: Option<(usize, usize)>` (anchor, cursor) works.
  - If no selection/cursor active, we need a default.
  - Change: Ensure `selection` is always `Some` when log window is open and focused (or at least valid when navigating).
  - `anchor == cursor` implies single line selection (cursor).
- Update `draw_log_window`:
  - Highlight the line at `cursor` always.
  - If `anchor != cursor`, highlight range.
  - Ensure `render_pane` is called with appropriate scroll offsets derived from cursor visibility logic (though `render_pane` takes `PaneState` which has scroll...).
  - **Wait**: `render_pane` uses `state.pane.scroll_vertical`. We need to UPDATE `state.pane.scroll_vertical` based on cursor position in `interaction.rs`. `view.rs` just renders based on state.

#### [MODIFY] [interaction.rs](file:///home/omarchy/Data/Projects/Personal/telegram-llm-tui-bugfix-log-window-wrapping-worktree/ui/src/interaction.rs)

- Refactor `handle_log_view_key`:
  - `Up`/`Down`/`j`/`k`:
    - Move `cursor` (the second element of `selection`).
    - If `Shift` is NOT held: Update `anchor` to match `cursor` (single line mode).
    - If `Shift` IS held: Keep `anchor` (range mode).
  - `Home`/`End`: Move `cursor` to 0 or max.
- Helper `ensure_log_cursor_visible`:
  - Check if `cursor` is within `[scroll, scroll + page_size)`.
  - If above `scroll`, `scroll = cursor`.
  - If below, `scroll = cursor - page_size + 1`.

## Verification Plan

### Automated Tests

- Run `cargo test -p ui` covering:
  - `interaction::tests::log_window_cursor_moves`: Verify cursor updates on keypress.
  - `interaction::tests::log_window_auto_scrolls`: Verify viewport scrolls to keep cursor visible.
  - `interaction::tests::log_selection_reset`: Verify non-shift nav resets selection anchor.

### Manual Verification

1. Open Log Window (`Ctrl+L`).
2. Verify last line is highlighted by default (or first? User didn't specify, but last makes sense for logs).
3. Press `Up`. Highlight should move up. View should scroll if at top.
4. Press `Shift+Down`. Highlight should extend downwards (range selection).
5. Press `Down` (no shift). Highlight should be single line at new position.
