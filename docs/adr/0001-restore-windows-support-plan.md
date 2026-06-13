# Implementation Plan: Restore Windows Support

**Status**: Ready for implementation
**Estimated effort**: 4–6 sessions of focused work
**ADR**: docs/adr/0001-restore-windows-support.md

---

## Phase 1 — Platform Abstraction Layer (no Windows code yet)

Create `#[cfg]` aliases and traits so that Linux and Windows can coexist via conditional compilation, without breaking any existing Linux behavior.

### Task 1.1 — Add `windows_vk` to config

**Files**: `src-tauri/src/config.rs`, `src-tauri/src/key_listener/mod.rs`, `src-tauri/src/config_store.rs`

- Add `windows_vk: Option<i32>` to `config::KeyListenerConfig` with `serde(default)` and `Option::default` (None).
- Add `windows_vk: Option<i32>` to `key_listener::KeyListenerConfig` and populate from config.
- Add `windows_vk: Option<i32>` to `key_capture::CaptureActivationResponse` (skip_serializing_if None).
- Verify existing tests still pass; add a round-trip test for `windows_vk`.

### Task 1.2 — Introduce `Recorder` trait

**Files**: new `src-tauri/src/recorder/trait.rs` (or mod.rs), `src-tauri/src/recorder/linux.rs`, `src-tauri/src/pipeline_context.rs`, `src-tauri/src/pipeline_builder.rs`

- Define trait `Recorder` with methods `start_recording(&mut self, buf: Arc<Mutex<Vec<u8>>>) -> Result<()>`, `stop_recording(&mut self) -> Result<()>`, `is_recording(&self) -> bool`.
- Implement `Recorder` for `PulseRecorder` (move existing method bodies behind the trait).
- Add `#[cfg(target_os = "linux")] pub type PlatformRecorder = PulseRecorder;` in `recorder/mod.rs`.
- Update `PipelineContext` and `PipelineBuilder` to use `PlatformRecorder` (the alias, which currently resolves to `PulseRecorder`) instead of the concrete type name.

### Task 1.3 — Introduce `PlatformListener` alias

**Files**: `src-tauri/src/key_listener/mod.rs`, `src-tauri/src/pipeline_context.rs`, `src-tauri/src/pipeline_builder.rs`

- Add `#[cfg(target_os = "linux")] pub type PlatformListener = X11Listener;` in `key_listener/mod.rs`.
- Update `PipelineContext` and `PipelineBuilder` to use `PlatformListener` instead of `X11Listener`.
- Verify Linux build and tests pass with no behavioral change.

### Task 1.4 — Gate `key_capture.rs` by platform

**Files**: `src-tauri/src/key_capture.rs`

- The current implementation is Linux-only (evdev). Wrap the entire file body with `#[cfg(target_os = "linux")]` or move to `key_capture/linux.rs` with a `#[cfg]` alias.
- Add a placeholder `#[cfg(target_os = "windows")] pub fn capture_activation_key_blocking()` that returns `Err("not implemented on Windows")`.

### Task 1.5 — Gate `tauri_overlay_window.rs` monitor query

**Files**: `src-tauri/src/tauri_overlay_window.rs`

- Extract the monitor query into a trait or a `#[cfg]`-gated helper.
- Wrap `xrandr_primary_monitor()` and `parse_xrandr_geometry()` with `#[cfg(target_os = "linux")]`.
- Add a `#[cfg(target_os = "windows")]` stub returning `Err` or a placeholder.

### Task 1.6 — Gate `pipeline_context.rs` bridge logic

**Files**: `src-tauri/src/pipeline_context.rs`

- The bridge logic (spawning a thread to poll `key_events`) is currently Linux-specific. With the `PlatformListener` alias in place, this code compiles for any type that satisfies the same interface.
- Verify the abstraction holds for a type that would return events via `tokio::mpsc::UnboundedReceiver` — this is already the case, so no additional changes needed here.

---

## Phase 2 — Windows Core Modules (pure Rust, no UI changes)

### Task 2.1 — Add Windows dependencies to `Cargo.toml`

**File**: `src-tauri/Cargo.toml`

Add Windows-only dependencies:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
] }
cpal = "0.15"
arboard = "3"
notify-rust = "4"
```

### Task 2.2 — Implement `key_listener/windows.rs`

**File**: new `src-tauri/src/key_listener/windows.rs`

- Implement `WindowsListener` struct wrapping the `WH_KEYBOARD_LL` hook.
- `start()` installs the hook, returns `(UnboundedReceiver<KeyEvent>, backend_name)`.
- Hook callback enqueues `KeyEvent { pressed: true/false }` on keydown/keyup.
- `drop()` uninstalls the hook.
- Add `#[cfg(target_os = "windows")] pub type PlatformListener = WindowsListener;` in `key_listener/mod.rs`.
- Unit tests: VK code mapping, event conversion.

### Task 2.3 — Implement `recorder/windows.rs`

**File**: new `src-tauri/src/recorder/windows.rs`

- Implement `WindowsRecorder` struct using `cpal`.
- `start_recording()` opens default input device, streams 16kHz/mono/i16 PCM into shared `Buffer`.
- `stop_recording()` stops the stream.
- Implement `Recorder` trait.
- Add `#[cfg(target_os = "windows")] pub type PlatformRecorder = WindowsRecorder;` in `recorder/mod.rs`.
- Unit tests: config conversion, error handling.

### Task 2.4 — Implement `output/windows.rs`

**File**: new `src-tauri/src/output/windows.rs`

- `write_clipboard(text)`: use `arboard::Clipboard::new().set_text(text)`.
- `notify_result(text)`: use `notify_rust::Notification::new().summary("altgo").body(text).show()`.
- Add `#[cfg(target_os = "windows")] pub use windows::{write_clipboard, notify_result};` in `output/mod.rs`.
- Unit tests: basic smoke tests (may need mock for CI if no display).

### Task 2.5 — Implement `key_capture/windows.rs`

**File**: new `src-tauri/src/key_capture/windows.rs`

- Install a temporary `WH_KEYBOARD_LL` hook, receive the first `WM_KEYDOWN`, record the VK code, uninstall.
- Return `CaptureActivationResponse { key_name: vk_to_name(vk), windows_vk: Some(vk), linux_evdev_code: None }`.
- `vk_to_name` helper: map common VK codes to human-readable names (`VK_RMENU` → `"Right Alt"`, etc.).
- Gate with `#[cfg(target_os = "windows")]` and wire into `key_capture/mod.rs`.

### Task 2.6 — Implement Windows monitor query

**File**: `src-tauri/src/tauri_overlay_window.rs`

- Add `#[cfg(target_os = "windows")] fn win32_primary_monitor_geometry() -> Option<(i32, i32, i32, i32)>` using `windows` crate `GetMonitorInfoW`.
- Update `primary_monitor_geometry()` to dispatch via `#[cfg]`.
- Unit tests: mock monitor info (struct-level test).

---

## Phase 3 — Config & Wiring

### Task 3.1 — Wire `windows_vk` into pipeline startup

**Files**: `src-tauri/src/key_listener/windows.rs`, `src-tauri/src/pipeline_context.rs`

- `WindowsListener::new()` reads `windows_vk` from `KeyListenerConfig`; if `None`, falls back to resolving `key_name` to a VK code.
- Update `pipeline_context.rs` log message to show correct backend name on Windows.

### Task 3.2 — Update `cmd.rs` platform dispatch

**File**: `src-tauri/src/cmd.rs`

- `capture_activation_key`: dispatch to `key_capture::capture_activation_key_blocking()` which is now `#[cfg]`-gated per platform.
- `copy_text`: dispatch to `output::write_clipboard()` which is now `#[cfg]`-gated.
- Verify existing IPC commands still work on Linux.

### Task 3.3 — Update `resource.rs` for Windows paths

**File**: `src-tauri/src/resource.rs`

- `bundled_bin()` currently checks `/usr/lib/altgo/bin/` (Linux installed) and exe-relative `bin/`. The exe-relative path already works on Windows. Add `%LOCALAPPDATA%\altgo\models\` path to model resolution if not already covered by `dirs::data_local_dir()`.

### Task 3.4 — Update `config.rs` save for Windows

**File**: `src-tauri/src/config.rs`

- The `save()` function has `#[cfg(unix)] std::os::unix::fs::PermissionsExt` for file permissions. On Windows, this should be a no-op (the `#[cfg(unix)]` block is already guarded). Verify no additional changes needed.

---

## Phase 4 — Packaging & CI

### Task 4.1 — Update `tauri.conf.json`

**File**: `src-tauri/tauri.conf.json`

- Add `"msi"` to `bundle.targets`: `["deb", "rpm", "msi"]`.
- Add `windows` section under `bundle`:
  ```json
  "windows": {
    "certificateThumbprint": null,
    "digestAlgorithm": "sha256",
    "webviewInstallMode": {
      "type": "embedBootstrapper"
    }
  }
  ```

### Task 4.2 — Create `download-deps-windows.ps1`

**File**: new `packaging/scripts/download-deps-windows.ps1`

- Reads `WHISPER_CPP_VERSION` and `FFMPEG_VERSION` from `packaging/scripts/versions.sh` (or a shared JSON/YAML, or hardcode with a comment pointing to versions.sh).
- Downloads `whisper-cli.exe` from whisper.cpp GitHub Release.
- Downloads `ffmpeg.exe` from gyan.dev or BtbN static build.
- Puts both in `target/deps/bin/`.

### Task 4.3 — Update CI release workflow

**File**: `.github/workflows/release.yml`

- Add `build-windows` job:
  - Runs on `windows-latest`.
  - Installs Rust stable, Node LTS.
  - Runs `.\packaging\scripts\download-deps-windows.ps1`.
  - Runs `cargo tauri build --bundles msi`.
  - Uploads `src-tauri/target/release/bundle/msi/*.msi` as artifact.
- Update `release` job to download Windows artifacts and include in checksums.

---

## Phase 5 — Testing & Verification

### Task 5.1 — Unit tests for Windows modules

- `key_listener/windows.rs`: VK mapping, event conversion, hook lifecycle (if testable without real hook).
- `recorder/windows.rs`: config construction, device enumeration error paths.
- `output/windows.rs`: smoke tests (clipboard write on Windows runner).
- `key_capture/windows.rs`: VK name mapping.
- `tauri_overlay_window.rs`: monitor geometry calculation with fake structs.

### Task 5.2 — Integration smoke test

- On Windows runner: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt`.
- Manual smoke: install MSI, verify overlay appears, recording starts/stops, clipboard works.

### Task 5.3 — Documentation updates

- `CLAUDE.md`: Add Windows architecture, build commands, platform tooling.
- `README.md`: Add Windows install instructions.
- `CONTRIBUTING.md`: Add Windows build environment setup.

---

## Dependency Order

```
Phase 1 (abstraction)  ──►  Phase 2 (Windows modules)  ──►  Phase 3 (wiring)  ──►  Phase 4 (packaging)  ──►  Phase 5 (testing)
     │                              │                              │
     └── Task 1.1–1.6              └── Task 2.1–2.6              └── Task 3.1–3.4
         (no Windows code)              (pure Windows code)            (cfg wiring)
```

Phases 1–3 are Rust-only and can be verified with `cargo check --target x86_64-pc-windows-msvc` (cross-compile check) even without a Windows machine. Phase 4 requires actual Windows CI. Phase 5 validates everything end-to-end.
