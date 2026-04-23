# Drop Windows Support — Linux Only

**Date**: 2026-04-23
**Status**: Approved

## Goal

Remove all Windows-specific code, files, dependencies, and packaging. altgo becomes a Linux-only application with no `#[cfg(target_os = "windows")]` conditional compilation.

## Motivation

- Simplify codebase — no cross-platform abstractions needed
- Reduce maintenance burden
- Focus on Linux (Ubuntu 20.04+) as the primary target
- Windows code has security issues (PowerShell injection) that are better removed than fixed

## Scope

### Files to Delete (5)

| File | Reason |
|------|--------|
| `src-tauri/src/output/windows.rs` | Windows clipboard/overlay module |
| `src-tauri/src/recorder/windows.rs` | Windows ffmpeg recorder |
| `src-tauri/src/key_listener/windows.rs` | Windows PowerShell key listener |
| `packaging/scripts/download-deps.ps1` | Windows dependency download |
| `packaging/windows/Product.wxs` | WiX MSI installer definition |

### Rust Source Changes (9 files)

| File | Changes |
|------|---------|
| `cmd.rs` | Remove `WinPoint`, FFI imports (`GetAsyncKeyState`, `GetCursorPos`), `is_key_pressed`, `resolve_vk_code`, `resolve_vk_for_pipeline`, `mouse_position_physical` (Windows), `position_overlay_tauri` fallback, all `#[cfg(target_os = "windows")]` blocks |
| `key_listener/mod.rs` | Remove `mod windows`, `#[cfg]` type alias — expose `linux::X11Listener` directly |
| `key_capture.rs` | Remove `#[cfg(windows)]` branches for key name resolution |
| `output/mod.rs` | Remove `mod windows`, `#[cfg]` type alias — expose `linux` module directly |
| `recorder/mod.rs` | Remove `mod windows`, `#[cfg]` type alias, `warmup_device` |
| `resource.rs` | Remove `.exe` suffix branch in `bundled_bin` |
| `transcriber.rs` | Remove Windows `cmd /C where` branch in `which_binary` |
| `lib.rs` | Remove `#[cfg(target_os = "windows")]` audio warmup thread |
| `pipeline.rs` | No changes needed (no Windows-specific code) |

### Build Config Changes

| File | Changes |
|------|---------|
| `src-tauri/Cargo.toml` | Remove `[target.'cfg(windows)'.dependencies]` section |
| `src-tauri/tauri.conf.json` | Remove `"msi"` from `bundle.targets` |
| `Makefile` | Remove `deps-windows` target and `.PHONY` entry |

### Documentation Updates (6 files)

| File | Changes |
|------|---------|
| `README.md` | Remove Windows install instructions, paths, features, `deps-windows` reference |
| `CLAUDE.md` | Remove Windows architecture, platform tooling, build commands |
| `CONTRIBUTING.md` | Remove Windows build environment, `cfg` usage for Windows |
| `docs-site/docs/architecture.mdx` | Remove Windows references |
| `docs-site/drafts/` | Delete Windows-related draft files |
| `CHANGELOG.md` | No change (history preserved) |

## Approach

1. Delete the 5 Windows-only files
2. For each of the 9 Rust source files, remove `#[cfg(target_os = "windows")]` blocks and simplify module exports
3. Update Cargo.toml, tauri.conf.json, Makefile
4. Update all documentation files
5. Verify: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt`, `make build`

## Verification

- `cargo check` — compiles without errors
- `cargo test` — all tests pass
- `cargo clippy -D warnings` — no warnings
- `cargo fmt --check` — formatting correct
- `make build` — full build succeeds
