# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Communication Language

始终使用中文与用户交流。代码、commit message、PR 描述等技术输出保持英文。

## 写作要求

所有面向人读的文本——CONTEXT.md、ADR、issue 评论、PR 描述、agent brief、triage notes、Sphinx 文档——遵守以下原则（原话引用）：

- **善于总结材料**：材料弄全弄准，去粗取精、去伪存真、由此及彼、由表及里，反映事物本质；不堆砌细节、不拼凑清单。
- **不用夸大的修饰词**：不写"权威""强大""完整""单一事实来源"之类的修饰，它们减损力量。
- **注意词语的逻辑界限**：相邻概念要划清（如"配置"与"运行规格"、"力模型"与"力模型聚合"），不混用、不模糊。
- **废话应当尽量除去**。
- **通俗、亲切，由小讲到大，由近讲到远，引人入胜**：先讲读者已知／当前的事物，再推到陌生／抽象的；忌一上来就宏大叙事或先搬死人、外国人。
- **与读者完全平等**：靠分析说服，不要装腔作势来吓人；老老实实办事。

## Project Overview

**altgo** is a desktop voice-to-text tool written in Rust, supporting **Linux** (Ubuntu 20.04+) and **Windows** (MSI via GitHub Releases). Hold the right Alt key to record speech, release to transcribe with **local whisper.cpp**, optionally polish via any **OpenAI-compatible LLM** API, then **write the result to the system clipboard** and show it in a **floating overlay** (overlay copy is a fallback if clipboard tools fail). Successful transcriptions (raw + displayed text) are **persisted as text-only history** in a local JSON file (`~/.config/altgo/history.json`); audio is never stored. Code may still include optional HTTP Whisper API paths for advanced use.

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

# make build: runs ensure-binary-deps (deps-linux), then
# cargo tauri build, then copies target/deps/bin/* into src-tauri/target/release/bin/
make build
make install                  # After build: altgo -> /usr/local/bin, deps -> /usr/lib/altgo/bin, config -> /etc/altgo/
```

### Windows Build

```powershell
# Equivalent of `make build` on Windows (downloads deps + cargo tauri build --bundles msi)
.\build.ps1
# or: pwsh packaging/scripts/build.ps1
# or: build.cmd (shim for systems without pwsh on PATH)

# Test/lint commands are the same as Linux
cargo test --manifest-path=src-tauri/Cargo.toml
cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings
```

## Architecture

Tauri desktop app with core logic in `src-tauri/src/`:

| Component | Path |
|-----------|------|
| **Tauri GUI** | `src-tauri/` + `frontend/` |
| **Core modules** | `src-tauri/src/` |

Core pipeline driven by keyboard events:

```
Key Listener → State Machine → Recorder → Transcriber → Polisher → Output (+ History JSON)
```

### Modules (in `src-tauri/src/`)

- **`lib.rs`** — Tauri app entry point, `AppState` struct (`config_path`, **`history_path`**, pipeline handle, pipeline status), run loop setup.
- **`cmd.rs`** — Tauri commands exposed to frontend via IPC: config (`get_config`, `save_config`, `capture_activation_key`), pipeline (`start_pipeline`, `stop_pipeline`, `get_status`), overlay (`copy_text`, `hide_overlay`), models (`list_models`, `download_model`, `delete_model`, `resolve_model`), **history** (`list_history`, `delete_history_entries`, `clear_history`, `polish_history_entry`). The voice pipeline (`run_pipeline`) appends a row when `raw_text` is non-empty, emits `history-updated` after a successful write, and prefers showing polished text only when it is non-empty after trim.
- **`history.rs`** — Append/list/delete/clear/update for `history.json` (camelCase JSON, `Mutex` for file I/O). Does not store audio.
- **`config.rs`** — TOML config loading with `serde(default)` for every field. API keys overridable via env vars (e.g. `ALTGO_POLISHER_API_KEY`; transcriber key if API engine used).
- **`state_machine.rs`** — 5-state enum (`Idle`, `PotentialPress`, `Recording`, `WaitSecondClick`, `ContinuousRecording`). Long-press records, double-click enters continuous mode. Uses `tokio::select!` to race key events vs timeouts.
- **`audio.rs`** — Thread-safe PCM buffer (`Mutex<Vec<u8>>`), WAV encode/decode (44-byte header + PCM).
- **`transcriber.rs`** — `WhisperApi` (HTTP multipart to OpenAI-compatible endpoint) and `LocalWhisper` (subprocess to `whisper-cli` binary).
- **`polisher.rs`** — LLM text polishing with 4 levels (`none`/`light`/`medium`/`heavy`). Retries with exponential backoff (3 attempts). Uses OpenAI-compatible chat API.
- **`pipeline.rs`** — Core processing pipeline (transcribe + polish). Caller handles output (overlay UI, optional clipboard inject, notifications).
- **`model.rs`** — whisper.cpp GGML model management (download, switch, storage in `~/.config/altgo/models/`).
- **`tray.rs`** — System tray configuration (show window, quit menu).
- **`resource.rs`** — Resource file management.
- **`key_capture.rs`** — One-shot activation key capture for Settings (Linux evdev; Windows WH_KEYBOARD_LL).
- **`key_listener/`** — Key detection (Linux: `xinput test-xi2` / Windows: `WH_KEYBOARD_LL` via `SetWindowsHookExW` on a dedicated message-pump thread).
- **`recorder/`** — Audio capture (Linux: `parecord` PulseAudio / Windows: `cpal` WASAPI; outputs 16kHz mono WAV; resamples via rubato if device rate differs).
- **`output/`** — Clipboard + notifications (Linux: `xclip`/`xsel`/`wl-copy` + `notify-send` / Windows: `arboard` clipboard + no-op notify; overlay handles display on Windows).

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
│   ├── History.tsx         # Transcription history (select / delete / clear / copy / polish one row)
│   └── Settings.tsx        # Settings page
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

**Subprocess-based system interaction (Linux)** — Platform integration on Linux shells out to CLI tools (`xinput`, `parecord`, `xclip`). This simplifies building and avoids native dependency complexity.

**Win32 API bindings (Windows)** — Windows uses the `windows` crate (0.61) for keyboard hooks (`WH_KEYBOARD_LL`) and monitor geometry (`GetMonitorInfoW`), `cpal` (0.17) for WASAPI audio capture, and `arboard` (3) for clipboard. These are `cfg(windows)`-only dependencies in `Cargo.toml`.

**Platform abstraction via `cfg` + traits** — Each platform module (`key_listener/`, `recorder/`, `output/`) uses `#[cfg(target_os)]` to select the concrete implementation. A `Platform*` type alias provides the default, and each module exposes a trait (`KeyListener`, `Recorder`, `Output`) so the pipeline can consume `Box<dyn Trait>` for testability.

**Async channel pipeline** — `tokio::sync::mpsc` channels decouple stages. Key events flow via unbounded channel, commands via bounded (capacity 16). Processing spawned as independent `tokio::spawn` tasks.

**Config** — Lives at `~/.config/altgo/altgo.toml`. Template at `configs/altgo.toml`. All fields have serde defaults so a partial config works.

**Transcription history** — `~/.config/altgo/history.json` (same directory as config). Entries: `id`, `createdAtMs`, `rawText`, `text`. The floating window and frontend listen for the **`history-updated`** event to refresh lists.

### System Requirements

**Linux**: `xinput`, `xmodmap`, `parecord`, `xclip`/`xsel`/`wl-copy`, `notify-send`

**Windows**: WebView2 Runtime (auto-installed by MSI if missing), microphone (WASAPI default device). No CLI tool dependencies — all platform integration uses Win32 APIs or bundled crates.

### Platform-specific Dependencies (Cargo.toml)

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61", features = [
    "Win32_UI_Input_KeyboardAndMouse", "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation", "Win32_System_Threading", "Win32_Graphics_Gdi",
] }
cpal = "0.17"
arboard = { version = "3", default-features = false }
```

Note: `cpal` 0.17 is pinned (not 0.18) to avoid breaking `windows-core` version conflicts with tao/tauri 2.10. See memory file `windows-cargo-version-pin`.

### Tauri GUI Development

Before first run, install frontend dependencies:
```bash
cd frontend && npm install
```

## Testing Notes

- Unit tests live in `#[cfg(test)]` modules within each source file.
- `config.rs`, `audio.rs`, `model.rs`, and `history.rs` have comprehensive tests.
- `transcriber.rs` and `polisher.rs` use `mockito` for HTTP-level mocking.
- Platform-specific modules have minimal tests (construction/smoke tests only).
- **Windows code is not tested in CI** (Linux-only CI runner). Windows-specific code paths are verified manually on a Windows machine. The release workflow builds MSI but does not run `cargo test` on Windows.

## Agent skills

### Issue tracker

Issues live in GitHub Issues (`cislunarspace/altgo`). Uses `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical triage labels with default names. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout (`CONTEXT.md` + `docs/adr/` at repo root). See `docs/agents/domain.md`.
