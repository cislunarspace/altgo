# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**altgo** is a cross-platform desktop voice-to-text tool written in Rust. Hold the right Alt key to record speech, release to transcribe (via Whisper API or local whisper.cpp), polish with an LLM, and paste from clipboard.

## Build & Test Commands

```bash
cargo build --release         # Optimized build
cargo test                    # Run all tests
cargo test -- --nocapture     # Run tests with println output
cargo test test_name          # Run tests matching pattern
cargo fmt                     # Format code
cargo fmt -- --check          # Check formatting (CI uses this)
cargo clippy -- -D warnings   # Lint with warnings as errors
make build                    # Build + copy binary to ./
make install                  # Install to /usr/local/bin + /etc/altgo/
```

CI runs on all three platforms (Linux, macOS, Windows) and checks: `fmt`, `clippy`, `build --release`, `test`.

## Architecture

Linear pipeline driven by keyboard events:

```
Key Listener → State Machine → Recorder → Transcriber → Polisher → Output
```

### Modules

- **`main.rs`** — CLI parsing (`clap`), wires all modules together, runs the main event loop. The `Transcriber` enum dispatches between API and local backends.
- **`config.rs`** — TOML config loading with `serde(default)` for every field. API keys overridable via `ALTGO_TRANSCRIBER_API_KEY` and `ALTGO_POLISHER_API_KEY` env vars.
- **`state_machine.rs`** — 5-state enum (`Idle`, `PotentialPress`, `Recording`, `WaitSecondClick`, `ContinuousRecording`). Long-press records, double-click enters continuous mode. Uses `tokio::select!` to race key events vs timeouts.
- **`audio.rs`** — Thread-safe PCM buffer (`Mutex<Vec<u8>>`), WAV encode/decode (44-byte header + PCM).
- **`transcriber.rs`** — `WhisperApi` (HTTP multipart to OpenAI-compatible endpoint) and `LocalWhisper` (subprocess to `whisper-cli` binary).
- **`polisher.rs`** — LLM text polishing with 4 levels (`none`/`light`/`medium`/`heavy`). Retries with exponential backoff (3 attempts). Uses OpenAI-compatible chat API.
- **`key_listener/`** — Platform-specific key detection. Linux: `xinput test-xi2`. macOS: CGEvent tap via inline Swift. Windows: PowerShell + `GetAsyncKeyState`.
- **`recorder/`** — Platform-specific audio capture. Linux: `parecord`. macOS: `sox`. Windows: `ffmpeg` (primary) or `sox` (fallback).
- **`output/`** — Platform-specific clipboard + notifications. Linux: `xclip`/`xsel`/`wl-copy` + `notify-send`. macOS: `pbcopy` + `osascript`. Windows: `clip.exe`/PowerShell + BurntToast.

### Key Patterns

**Cross-platform dispatch** — Each platform module (`key_listener`, `recorder`, `output`) uses `#[cfg(target_os = ...)]` in `mod.rs` to expose a single type alias (`PlatformListener`, `PlatformRecorder`, etc.). No trait objects; statically dispatched.

**Subprocess-based system interaction** — All platform integration shells out to CLI tools rather than using FFI. This simplifies cross-compilation.

**Async channel pipeline** — `tokio::sync::mpsc` channels decouple stages. Key events flow via unbounded channel, commands via bounded (capacity 16). Processing spawned as independent `tokio::spawn` tasks.

**Config** — Lives at `~/.config/altgo/altgo.toml`. Template at `configs/altgo.toml`. All fields have serde defaults so a partial config works.

### Platform System Requirements

- **Linux**: `xinput`, `xmodmap`, `parecord`, `xclip`/`xsel`/`wl-copy`, `notify-send`
- **macOS**: `sox`, Swift CLI tools, `pbcopy`, `osascript`
- **Windows**: `ffmpeg` or `sox`, PowerShell

## Testing Notes

- Unit tests live in `#[cfg(test)]` modules within each source file.
- `config.rs` and `audio.rs` have comprehensive tests.
- `transcriber.rs` and `polisher.rs` use `mockito` for HTTP-level mocking.
- Platform-specific modules have minimal tests (construction/smoke tests only).
- No integration test directory (`tests/`) exists yet.
