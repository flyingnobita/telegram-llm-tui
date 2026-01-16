# Changes

- Jan-16, 2026 - 08:28 PM +08 - Moved log dir to repo root.
- Jan-16, 2026 - 04:39 PM +0800 - Moved history sync to the background so TUI starts.
- Jan-16, 2026 - 04:26 PM +0800 - Added horizontal scrolling and scrollbars
  to the message pane.
- Jan-16, 2026 - 04:24 PM +0800 - Rendered service messages instead of warnings.
- Jan-16, 2026 - 04:09 PM +0800 - Fixed message pane scrolling for
  multi-line messages so long bodies can be viewed.
- Jan-16, 2026 - 04:02 PM +0800 - Added startup history sync (default 100 per chat).
- Jan-16, 2026 - 03:49 PM +0800 - Cached user display names so message
  authors render by name instead of raw IDs.
- Jan-16, 2026 - 03:37 PM +0800 - Added a toggleable, scrollable log window
  overlay with a default hotkey and escape close.
- Jan-15, 2026 - 10:52 PM +0800 - Widened chat list and added scrolling.
- Jan-15, 2026 - 10:27 PM +0800 - Synced dialog titles so chat list shows names.
- Jan-15, 2026 - 09:54 PM +0800 - Cleared message pane before render.
- Jan-15, 2026 - 09:51 PM +0800 - Highlighted the focused TUI pane border in red.
- Jan-15, 2026 - 09:34 PM +0800 - Wired keymap config and viewport paging.
- Jan-15, 2026 - 09:15 PM +0800 - Added runtime TUI loop for layout and input.
- Jan-09, 2026 - 01:40 AM +0800 - Added input ergonomics state and keymap handlers.
- Jan-09, 2026 - 01:14 AM +0800 - Wired cache data into UI state bridge and tests.
- Jan-09, 2026 - 12:55 AM +0800 - Added layout v1 with chat list, composer, overlays.
- Jan-09, 2026 - 12:40 AM +0800 - Implemented sqlite cache persistence and wiring.
- Jan-09, 2026 - 12:17 AM +0800 - Noted TBD cache limits in 2.4 SDD.
- Jan-09, 2026 - 12:11 AM +0800 - Documented sqlite cache store and 2.4 plans.
- Jan-08, 2026 - 11:31 PM +08 - Added send pipeline with retry and queueing.