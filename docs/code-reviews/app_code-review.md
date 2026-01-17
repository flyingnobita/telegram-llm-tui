# January 17, 2026

1. [ ] High - `log_content` is parsed but not enforced, so `info!(?event, ...)`
   logs full event payloads including message text. This can leak sensitive
   content and makes the config misleading.
2. [ ] Medium - `prompt_secret` echoes input because it calls `prompt_line`, so
   2FA passwords are visible in the terminal.
3. [ ] Medium - The log viewer reads the entire log file on every refresh and
   truncates in memory, which can stall the UI for larger log files.
