# Changes

- Jan-16, 2026 - 04:16 PM +0800 - Added `log_window_max_lines` to config for
  the log window tail size.
- Jan-16, 2026 - 04:10 PM +0800 - Loaded the log window from the primary log
  file and refreshed it while the window is open.
- Jan-16, 2026 - 04:06 PM +0800 - Switched the log window hotkey to `l` for
  open and close behavior.
- Jan-16, 2026 - 03:54 PM +0800 - Suppressed console logging while the TUI
  runs to prevent screen corruption.
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
