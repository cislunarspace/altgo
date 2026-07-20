# ADR-0001: Restore Windows Support as Primary Platform

**Date**: 2026-06-12
**Status**: Accepted
**Supersedes**: (original file removed — was docs/superpowers/specs/2026-04-23-drop-windows-support-design.md)

## Context

altgo was originally a cross-platform application supporting Linux and Windows. In April 2026, Windows support was removed to reduce maintenance burden and simplify the codebase. The original Windows implementation used PowerShell subprocesses for key listening, clipboard, and notifications, which introduced security concerns (PowerShell injection) and reliability issues (dependence on Windows Execution Policy and anti-malware interference).

The decision to restore Windows support was driven by user demand for Windows as a first-class target platform, with a commitment to long-term maintenance.

## Decision

We will restore Windows support as a **first-class target platform**, using **native Windows APIs** instead of the previously-removed PowerShell subprocess approach.

### Target Platform

- **OS**: Windows 10 1809+ / Windows 11
- **Architecture**: x86_64 only (ARM64 deferred)
- **Packaging**: MSI via Tauri bundler

### Technology Choices

| Component | Linux | Windows |
|-----------|-------|---------|
| Key listening | `xinput test-xi2` + `evtest` (subprocess) | `WH_KEYBOARD_LL` (Win32 low-level keyboard hook) |
| Audio recording | `parecord` (subprocess) | `cpal` crate (native Rust) |
| Clipboard | `xclip`/`xsel`/`wl-copy` (subprocess) | `arboard` crate (native Rust) |
| Notifications | `notify-send` (subprocess) | Tauri overlay (no notification library) |
| Overlay positioning | `xrandr` (subprocess) | `GetMonitorInfoW` (Win32 API) |
| Activation key capture | `evtest` + evdev | `WH_KEYBOARD_LL` hook in capture mode |
| Monitor geometry | `xrandr` query | `MonitorFromWindow` + `GetMonitorInfoW` |

### Configuration

- New field `windows_vk: Option<i32>` in key listener config (stores Windows virtual key code)
- Existing `key_name` field used as fallback for cross-platform config portability
- Config/history path: `%APPDATA%\altgo\config.toml` and `%APPDATA%\altgo\history.json`
- Model path: `%LOCALAPPDATA%\altgo\models\`

### Binary Dependencies

- `whisper-cli.exe` and `ffmpeg.exe` downloaded via `packaging/scripts/download-deps-windows.ps1`
- Version unified via `packaging/scripts/versions.sh`

### CI/CD

- PR/push: Linux only (fast feedback)
- Tag push (release): Linux + Windows MSI

### Test Strategy

- Abstract platform APIs behind traits for unit testability
- Unit tests cover: event conversion, VK mapping, geometry calculation, config round-trip, error handling
- Integration smoke tests on Windows runner
- E2E deferred to second phase

## Consequences

- **Supersedes** the April 2026 "Drop Windows Support" ADR, which is now historical
- Introduces `windows` crate as a Windows-only dependency for Win32 FFI
- Introduces `cpal` (Windows only) and `arboard` (cross-platform); no notification library — Windows results are shown by the Tauri overlay
- Requires `#[cfg(target_os = "windows")]` guards and platform-specific module files
- Windows binary dependencies require download script and CI caching
- Linux code remains unchanged; no behavioral regression

## Alternatives Considered

- **Subprocess-based Windows support** (rejected): Would reintroduce PowerShell injection risks and the reliability issues that motivated the original removal.
- **Tauri plugin for clipboard/notifications** (rejected): Insufficient control over platform behavior and would create tight coupling to Tauri's plugin API.
- **Direct WASAPI recording** (rejected): `cpal` provides sufficient abstraction for altgo's simple recording needs without the complexity of raw COM/unsafe code.
