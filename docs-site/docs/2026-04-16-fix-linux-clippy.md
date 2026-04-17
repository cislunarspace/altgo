# Fix Linux clippy: Windows-only functions in cli.rs

## Goal
Make `cli.rs` compile on Linux/macOS by gating Windows-only output functions and providing cross-platform fallbacks.

## Tasks
- [x] 1. Add `show_recording_window` / `close_recording_window` stubs on Linux and macOS (no-op)
- [x] 2. Add `output_text` function to Linux and macOS modules (write to clipboard + notify)
- [x] 3. Re-export new functions from `output/mod.rs` for Linux and macOS
- [x] 4. ~~Gate Windows-only calls in `cli.rs`~~ — not needed, `cli.rs` now uses uniform API across platforms
- [x] 5. Verify with `cargo clippy`

## Notes
- `output_text` on Windows does cursor injection + floating window; on Linux/macOS it should just write clipboard + notify
- `show_recording_window` / `close_recording_window` are Windows-only UI features; no-op on other platforms
