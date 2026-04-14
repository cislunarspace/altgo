# Architecture

> altgo is a cross-platform desktop voice-to-text tool written in Rust. Hold the right Alt key to record speech, release to transcribe (via Whisper API or local whisper.cpp), polish with an LLM, and paste from clipboard.

## Core Interaction

| Operation | Trigger | Behavior |
|-----------|---------|----------|
| **Long press right Alt** | Hold to speak, release to end | Press → Recording starts; Release → Recording ends → Transcription → Polishing → Output |
| **Double-click right Alt** | Quick double press to enter, press again to exit | Enters continuous recording mode; single press to stop → same subsequent flow |

### Polishing Levels

| Level | Effect | Use Case |
|-------|--------|----------|
| `none` | No polishing, output ASR raw text | Scenarios requiring original speech |
| `light` | Correct punctuation and obvious typos | Daily chat |
| `medium` | Fix grammar, optimize expression for smoothness | Formal documents, emails |
| `heavy` | Rewrite into structured书面 text | Reports, articles |

---

## Tech Stack

```
Language:     Rust (2021 edition)
Build:        cargo
Key Listener: Platform-specific (xinput on Linux, CGEvent tap on macOS, PowerShell on Windows)
Audio Record: Platform-specific (parecord on Linux, sox on macOS, ffmpeg on Windows)
ASR Engine:   OpenAI Whisper API (preferred) / whisper.cpp local (fallback)
LLM Polish:   OpenAI-compatible HTTP API (compatible with OpenAI/Claude/Ollama/vLLM)
Clipboard:    Platform-specific (xclip/xsel/wl-copy on Linux, pbcopy on macOS, clip.exe on Windows)
Notifications: Platform-specific (notify-send on Linux, osascript on macOS, BurntToast on Windows)
Config:       TOML
Logging:      tracing + tracing-subscriber
```

---

## Architecture

### Data Flow

```
User Key Press → State Machine → Start/Stop Recording
                                    ↓
                              Audio Buffer (PCM → WAV)
                                    ↓
                              ASR Transcription → Raw Text
                                    ↓
                              LLM Polish → Final Text
                                    ↓
                    ┌───────────────┴───────────────┐
                Write to Clipboard              Desktop Notification
```

### Concurrency Model

```
main (async main)
  └── key_listener task ──channel: KeyEvent──→ state_machine task
                                                       │ channel: Command
                                                       ↓
                                                  recorder task
                                                       │ channel: AudioData
                                                       ↓
                                                  transcriber task → polisher task → output task
```

Modules communicate via `tokio::sync::mpsc` channels. Processing stages are spawned as independent `tokio::spawn` tasks.

### Key State Machine

```
Idle
  │ Press
  ↓
PotentialPress
  │ Release(> long press threshold)     │ Release(< long press threshold)
  ↓                                      ↓
Recording (long press recording)       WaitSecondClick (waiting for double click)
  │ Release                              │ Press(within interval)   │ Timeout
  ↓                                      ↓                          ↓
Processing                           ContinuousRecording        Idle (ignore single press)
                                      │ Press
                                      ↓
                                    Processing
```

---

## Directory Structure

```
altgo/
├── src/
│   ├── main.rs                 # Entry point, CLI parsing, wires all modules
│   ├── config.rs               # TOML config loading with serde defaults
│   ├── state_machine.rs         # 5-state enum for key press handling
│   ├── audio.rs                # Thread-safe PCM buffer, WAV encode/decode
│   ├── transcriber.rs           # WhisperApi (HTTP) and LocalWhisper (subprocess)
│   ├── polisher.rs             # LLM text polishing with exponential backoff retry
│   ├── key_listener/           # Platform-specific key detection
│   │   ├── mod.rs              # Linux implementation (xinput test-xi2)
│   │   ├── macos.rs            # macOS implementation (CGEvent tap via Swift)
│   │   └── windows.rs          # Windows implementation (PowerShell + GetAsyncKeyState)
│   ├── recorder/               # Platform-specific audio capture
│   │   ├── mod.rs              # Linux (parecord)
│   │   ├── macos.rs            # macOS (sox)
│   │   └── windows.rs          # Windows (ffmpeg primary, sox fallback)
│   └── output/                 # Platform-specific clipboard + notifications
│       ├── mod.rs              # Linux (xclip/xsel/wl-copy + notify-send)
│       ├── macos.rs            # macOS (pbcopy + osascript)
│       └── windows.rs          # Windows (clip.exe + BurntToast)
├── configs/
│   └── altgo.toml              # Default configuration template
├── msi/
│   └── Product.wxs             # WiX MSI configuration
├── Cargo.toml
├── Cargo.lock
├── Makefile
└── README.md
```

---

## Module Details

### config.rs

TOML config loading with `serde(default)` for every field. API keys overridable via `ALTGO_TRANSCRIBER_API_KEY` and `ALTGO_POLISHER_API_KEY` environment variables.

### state_machine.rs

5-state enum (`Idle`, `PotentialPress`, `Recording`, `WaitSecondClick`, `ContinuousRecording`). Long-press records, double-click enters continuous mode. Uses `tokio::select!` to race key events vs timeouts.

### audio.rs

Thread-safe PCM buffer (`Mutex<Vec<u8>>`), WAV encode/decode (44-byte header + PCM).

### transcriber.rs

- `WhisperApi`: HTTP multipart to OpenAI-compatible endpoint
- `LocalWhisper`: Subprocess to `whisper-cli` binary

### polisher.rs

LLM text polishing with 4 levels (`none`/`light`/`medium`/`heavy`). Retries with exponential backoff (3 attempts). Uses OpenAI-compatible chat API.

### key_listener/, recorder/, output/

Platform-specific modules using `#[cfg(target_os = "...")]` in `mod.rs` to expose single type aliases (`PlatformListener`, `PlatformRecorder`, `PlatformOutput`). Statically dispatched, no trait objects.

---

## Future Optimizations

1. Local whisper.cpp integration (offline, low latency)
2. evdev key listener (Wayland native)
3. VAD voice activity detection (auto-detect speech end)
4. System tray icon (show status)
5. Custom hotkeys
6. DEB/RPM packaging improvements
