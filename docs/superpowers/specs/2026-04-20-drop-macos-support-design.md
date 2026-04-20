# Drop macOS Support — Design Spec

## Overview

Remove all macOS-specific code and conditionals from the altgo project, focusing development on Windows and Linux only.

## Scope

- Delete 3 macOS platform files
- Remove `#[cfg(target_os = "macos")]` conditionals from 3 `mod.rs` files
- Update `cmd.rs` and `resource.rs` macOS-related code
- Update documentation (CONTRIBUTING.md, CLAUDE.md)
- Regenerate Tauri schemas (generated files, auto-overwritten on next build)

## Files to Delete

| File | Purpose |
|------|---------|
| `src-tauri/src/key_listener/macos.rs` | macOS key listener via CGEvent tap + Swift |
| `src-tauri/src/recorder/macos.rs` | macOS audio recorder via sox/ffmpeg |
| `src-tauri/src/output/macos.rs` | macOS clipboard (pbcopy) and notifications (osascript) |

## Source Files to Modify

### `src-tauri/src/key_listener/mod.rs`
- Remove `#[cfg(target_os = "macos")] mod macos;`
- Remove `#[cfg(target_os = "macos")] pub type PlatformListener = macos::MacOSListener;`

### `src-tauri/src/recorder/mod.rs`
- Remove `#[cfg(target_os = "macos")] mod macos;`
- Remove `#[cfg(target_os = "macos")] pub type PlatformRecorder = macos::SoxRecorder;`

### `src-tauri/src/output/mod.rs`
- Remove `#[cfg(target_os = "macos")] mod macos;`
- Remove macOS re-exports block

### `src-tauri/src/cmd.rs`
- `get_focused_monitor_info()` currently returns `None` on non-Linux (macOS + Windows) via `#[cfg(not(target_os = "linux"))]`. Keep the `None` fallback for Windows; remove macOS from the comment scope.
- The `#[cfg(not(target_os = "windows"))]` block covering Linux + macOS key listener path remains valid for Linux only.

### `src-tauri/src/resource.rs`
- The `#[cfg(not(target_os = "linux"))]` block handling bundled binary lookup for macOS + Windows should remain, but comment should reflect Windows only now.

## Documentation to Update

| File | Change |
|------|--------|
| `CONTRIBUTING.md` | Remove `#[cfg(target_os = "macos")]` from cfg example |
| `CLAUDE.md` | Remove macOS from "Platform System Requirements" section |
| Various design docs in `docs/superpowers/specs/` | Remove macOS mentions (descriptive only, non-functional) |

## Tauri Schema Files

Files in `src-tauri/gen/schemas/` (e.g., `desktop-schema.json`, `windows-schema.json`) contain macOS references. These are generated files — they will be overwritten automatically on the next `cargo build` or `tauri build`. No manual changes needed.

## What Does NOT Change

- CI workflows (`.github/workflows/`) — no macOS jobs exist
- Cargo.toml — no macOS-specific dependencies
- Any Windows-specific code
- Linux-specific code

## Rollback

If macOS support needs to be restored, the three deleted `.rs` files can be recovered from git history (`git checkout <commit> -- <file>`).
