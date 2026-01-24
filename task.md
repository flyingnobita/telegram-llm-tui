# Debugging Clipboard Error

- [ ] Create reproduction script `scripts/reproduce_clipboard.rs` <!-- id: 0 -->
- [ ] Run reproduction script to capture error <!-- id: 1 -->
- [ ] Analyze error and fix dependencies or implement fallback <!-- id: 2 -->
- [x] Implement persistent clipboard worker thread in `app/src/tui.rs` <!-- id: 4a -->
- [x] Route copy actions to clipboard worker <!-- id: 5a -->
- [x] Refactor clipboard logic to `ui` crate for testability <!-- id: 7 -->
- [x] Create integration test `tests/clipboard_verification.rs` (Copy -> Paste to File) <!-- id: 8 -->
- [x] Refine test to use realistic log selection logic <!-- id: 8b -->
- [x] Reproduce failure in test <!-- id: 9 -->
- [x] Fix clipboard persistence (Investigate `wait()` or event loop) <!-- id: 10 -->
- [x] Verify fix with test <!-- id: 11 -->
- [x] Add debug logging to live app (`get_selected_log_text` and worker) <!-- id: 12 -->
- [x] Implement command-line fallback (`xclip`/`wl-copy`) <!-- id: 13 -->
- [x] Verify fallback strategy <!-- id: 14 -->
