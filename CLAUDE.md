# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**altgo** is a cross-platform desktop voice-to-text tool written in Rust (product docs target **Linux** first, Ubuntu 20.04 tested). Hold the right Alt key to record speech, release to transcribe with **local whisper.cpp**, optionally polish via any **OpenAI-compatible LLM** API, and show results in a **floating overlay** for user-initiated copy (not auto-clipboard by default in end-user messaging). Code may still include optional HTTP Whisper API paths for advanced use.

## Build & Test Commands

```bash
# Rust only (no GUI)
cargo build --release --manifest-path=src-tauri/Cargo.toml
cargo test --manifest-path=src-tauri/Cargo.toml
cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings

# Tauri GUI mode
cargo tauri dev               # Dev mode (frontend dev server + desktop window)
cargo tauri build            # Production GUI build

# make build: runs ensure-binary-deps (may run deps-linux / deps-windows), then
# cargo tauri build, then copies target/deps/bin/* into src-tauri/target/release/bin/
make build
make install                  # After build: altgo -> /usr/local/bin, deps -> /usr/lib/altgo/bin, config -> /etc/altgo/
```

## Architecture

Tauri desktop app with core logic in `src-tauri/src/`:

| Component | Path |
|-----------|------|
| **Tauri GUI** | `src-tauri/` + `frontend/` |
| **Core modules** | `src-tauri/src/` |

Core pipeline driven by keyboard events:

```
Key Listener → State Machine → Recorder → Transcriber → Polisher → Output
```

### Modules (in `src-tauri/src/`)

- **`lib.rs`** — Tauri app entry point, `AppState` struct, run loop setup.
- **`cmd.rs`** — Tauri commands exposed to frontend via IPC (get_config, save_config, capture_activation_key, start_pipeline, stop_pipeline, get_status, copy_text, hide_overlay).
- **`config.rs`** — TOML config loading with `serde(default)` for every field. API keys overridable via env vars (e.g. `ALTGO_POLISHER_API_KEY`; transcriber key if API engine used).
- **`state_machine.rs`** — 5-state enum (`Idle`, `PotentialPress`, `Recording`, `WaitSecondClick`, `ContinuousRecording`). Long-press records, double-click enters continuous mode. Uses `tokio::select!` to race key events vs timeouts.
- **`audio.rs`** — Thread-safe PCM buffer (`Mutex<Vec<u8>>`), WAV encode/decode (44-byte header + PCM).
- **`transcriber.rs`** — `WhisperApi` (HTTP multipart to OpenAI-compatible endpoint) and `LocalWhisper` (subprocess to `whisper-cli` binary).
- **`polisher.rs`** — LLM text polishing with 4 levels (`none`/`light`/`medium`/`heavy`). Retries with exponential backoff (3 attempts). Uses OpenAI-compatible chat API.
- **`pipeline.rs`** — Core processing pipeline (transcribe + polish). Caller handles output (overlay UI, optional clipboard inject, notifications).
- **`model.rs`** — whisper.cpp GGML model management (download, switch, storage in `~/.config/altgo/models/`).
- **`tray.rs`** — System tray configuration (show window, quit menu).
- **`resource.rs`** — Resource file management.
- **`key_capture.rs`** — One-shot activation key capture for Settings (Linux evdev / Windows VK resolution).
- **`key_listener/`** — Platform-specific key detection. Linux: `xinput test-xi2`. Windows: PowerShell + `GetAsyncKeyState`.
- **`recorder/`** — Platform-specific audio capture. Linux: `parecord`. Windows: `ffmpeg`.
- **`output/`** — Platform-specific clipboard + notifications. Linux: `xclip`/`xsel`/`wl-copy` + `notify-send`. Windows: `clip.exe`/PowerShell + BurntToast.

### Frontend Structure (`frontend/src/`)

```
├── App.tsx                 # App entry
├── main.tsx                # React render entry
├── ThemeContext.tsx        # Theme provider
├── theme.ts                # Theme tokens / persistence
├── overlay.tsx             # Floating window component
├── overlay.css             # Overlay styles (imports overlay-base, motion)
├── components/
│   ├── ui/                 # Base UI components (Input, Button, Card)
│   ├── Layout.tsx          # Layout component
│   └── StatusIndicator.tsx # Status indicator
├── pages/
│   ├── Home.tsx            # Home page
│   └── Settings.tsx      # Settings page
├── hooks/
│   └── useTauri.ts         # Tauri integration hook
├── i18n/                   # Internationalization
└── styles/
    ├── global.css
    ├── components.css
    ├── design-system.css
    ├── design-tokens.css   # Design tokens
    ├── motion.css          # Motion / transitions
    └── overlay-base.css    # Shared overlay layout
```

### Key Patterns

**Cross-platform dispatch** — Each platform module (`key_listener`, `recorder`, `output`) uses `#[cfg(target_os = ...)]` in `mod.rs` to expose a single type alias (`PlatformListener`, `PlatformRecorder`, etc.). No trait objects; statically dispatched.

**Subprocess-based system interaction** — All platform integration shells out to CLI tools rather than using FFI. This simplifies cross-compilation.

**Async channel pipeline** — `tokio::sync::mpsc` channels decouple stages. Key events flow via unbounded channel, commands via bounded (capacity 16). Processing spawned as independent `tokio::spawn` tasks.

**Config** — Lives at `~/.config/altgo/altgo.toml`. Template at `configs/altgo.toml`. All fields have serde defaults so a partial config works.

### Platform System Requirements

- **Linux**: `xinput`, `xmodmap`, `parecord`, `xclip`/`xsel`/`wl-copy`, `notify-send`
- **Windows**: `ffmpeg`, PowerShell

### Tauri GUI Development

Before first run, install frontend dependencies:
```bash
cd frontend && npm install
```

## Testing Notes

- Unit tests live in `#[cfg(test)]` modules within each source file.
- `config.rs`, `audio.rs`, and `model.rs` have comprehensive tests.
- `transcriber.rs` and `polisher.rs` use `mockito` for HTTP-level mocking.
- Platform-specific modules have minimal tests (construction/smoke tests only).
