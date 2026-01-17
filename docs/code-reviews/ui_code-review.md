# 2026-01-17

1. High: `InputState` treats `cursor` as a character index but uses byte offsets
   (`text.len()`, `String::insert`, `String::remove`). With non-ASCII input,
   cursor movement and backspace can panic or corrupt the cursor position.
   Consider storing byte offsets or using grapheme indices via
   `unicode-segmentation`, and update clamp and move logic accordingly.
   (`ui/src/input.rs:10`)
2. Medium: The global log hotkey intercepts plain `l` before focus handling, so
   typing `l` in the composer opens logs, and Vim `l` for horizontal movement
   never reaches handlers. Gate this behind a modifier or only when focus is not
   `Composer` and style is not `Vim`. (`ui/src/interaction.rs:44`)
