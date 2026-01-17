# SDD 3.2.3 - Composer cursor blink

## 1.0 Idea

1. [x] (1.1.1) Show a blinking underscore at the composer cursor.

## 2.0 Structured spec

### 2.1 Behavior

1. [x] (2.1.1) When the composer is focused and no overlay is open, render an
   underscore at the cursor position.
2. [x] (2.1.2) Blink cadence is 500 ms, toggling visibility without changing
   the stored input text.
3. [x] (2.1.3) Cursor position is clamped to UTF-8 character boundaries for
   display.

### 2.2 Constraints

1. [x] (2.2.1) Do not mutate the stored input text to render the cursor.
2. [x] (2.2.2) Use an ASCII underscore as the cursor glyph.

## 3.0 Implementation plan

1. [x] (3.1.1) Track a composer cursor blink flag in UiState.
2. [x] (3.1.2) Toggle the blink flag in the TUI loop on a 500 ms interval.
3. [x] (3.1.3) Render composer text with an inserted underscore when visible.

## 4.0 Test plan

1. [x] (4.1.1) Render test shows underscore when focused and blink flag is on.
2. [x] (4.1.2) Render test hides underscore when blink flag is off.

## 5.0 Code plan

1. [x] (5.1.1) Update composer rendering in ui view.
2. [x] (5.1.2) Add tests in ui test harness.
3. [x] (5.1.3) Update specs and changelog.
