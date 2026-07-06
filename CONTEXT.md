# Domain Glossary

This file defines the vocabulary used throughout the altgo codebase. Use these terms exactly in code, documentation, and architectural discussions.

## Core Pipeline

**Voice Pipeline**
The end-to-end processing chain: key press → recording → transcription → polishing → output. Driven by the state machine; managed at runtime by `PipelineController`.

**Pipeline Status**
The lifecycle phase of the voice pipeline at any instant: `Idle`, `Recording`, `Processing`, or `Done`. Represented as `PipelineStatus` enum throughout the Rust backend; serialised to lowercase string on the IPC boundary for the frontend.

**PipelineController**
Owns the pipeline run handle and the shared `PipelineStatus` arc. Responsible for start, stop, and restart. Does not know how to spawn a pipeline — callers inject a spawn closure so this module stays free of Tauri and sink dependencies. Lives in `pipeline_controller.rs`.

**PipelineSink**
The trait that receives events from the running pipeline: status changes, progress, errors, and transcription results. `TauriPipelineSink` in `tauri_sink.rs` is the single concrete adapter in production use. The transcription-result path delegates business work (clipboard write + history append) to a `TranscriptionDispatch` trait object injected at construction; the sink itself only emits Tauri events and toggles overlay state.

## Configuration

**ConfigStore**
Holds the live in-memory config behind a `Mutex` alongside the backing file path. Exposes `snapshot`, `snapshot_blocking`, and `apply_patch`. All config mutations go through `apply_patch`, which validates and persists atomically. Lives in `config_store.rs`.

**ConfigPatch**
A partial update to the config: all fields optional, absent fields left unchanged. The `linux_evdev_code` field uses a three-state deserialiser to distinguish absent (no change) from JSON `null` (clear the stored code). This is the type accepted by `save_config` over IPC.

## History

**HistoryStore**
Wraps the history JSON file and exposes named operations: `list`, `count`, `append`, `delete`, `clear`, `get`, `update_text`. Callers never handle the file path or module-private helpers. Lives in `history.rs`. Each instance is cheap to clone (it contains only a `PathBuf`).

**HistoryEntry**
A single transcription record: `id`, `createdAtMs`, `rawText` (Whisper output), `text` (polished or same as raw). Audio is never stored.

## Output

**Overlay**
The floating status window shown during recording, processing, and result display. Positioned on the primary monitor via `xrandr` geometry. Managed by `TauriPipelineSink` on status transitions through an `OverlaySink` abstraction; `TauriPipelineSink` only describes intent ("recording" / "processing" / "hidden" / "done"), the overlay manager translates that to window size/position/show/hide.

**Polisher**
The optional LLM post-processing step. Controlled by `PolishLevel` (`none`/`light`/`medium`/`heavy`). Communicates with any OpenAI-compatible chat API.

**PromptStore**
Manages prompt template files for the Polisher: loads from `resources/prompts/`, composes base + level-specific suffix into complete system prompts, validates on first use, and hot-reloads when files change. Validation errors degrade gracefully—polishing continues with raw transcription text and an overlay error message.

**Prompt Template**
A text file in `resources/prompts/`: either `base.txt` (shared instruction + Chinese writing guidance) or `{level}-suffix.txt` (level-specific instruction appended to base). Runtime composition produces the complete system prompt sent to the LLM.

**System Prompt**
The complete prompt text sent to the LLM for polishing, composed at runtime from `base.txt` + `{level}-suffix.txt`. Cached in memory and reloaded when template files change.

## Recording

**Recorder Output Format**
The audio format the Voice Pipeline expects a recorder to return as WAV bytes. The configured `sample_rate` describes the target recorder output sample rate (default 16kHz); output is always mono 16-bit PCM. Platform recorders may adapt native device formats into this target shape where practical.

**Windows Recording Format Adaptation**
On Windows, the recorder captures from the default WASAPI input device and adapts common device sample formats (`i16`, `u16`, `f32`) into signed 16-bit PCM. Multi-channel input is downmixed to mono when the target output is mono. If the device cannot capture at the target sample rate, the recorder resamples the captured audio to the target rate before returning WAV bytes.

## Key Input

**KeyListener**
The interface for continuously monitoring the configured activation key and emitting `KeyEvent`s while the pipeline is running. Implemented by platform adapters (`X11Listener` on Linux, `WindowsListener` on Windows). The pipeline consumes it as `Box<dyn KeyListener>` so the same lifecycle code runs on both platforms.
_Avoid_: key listener (lowercase, when referring to the concept), platform listener.

**KeyCapture**
The interface for one-shot capture of any physical key during Settings configuration, returning the key identifiers needed by `KeyListenerConfig` (`key_name`, `linux_evdev_code`, `windows_vk`). Implemented by platform adapters that reuse the same low-level input mechanism as `KeyListener` but expose a synchronous blocking API.
_Avoid_: capture mode, key capture mode.

**Activation Key**
The physical key held to start recording. Configured per-device as either an X11 keysym name (`key_name`) or an evdev scancode (`linux_evdev_code`). The evdev path is preferred on Wayland. On Windows, configured via Windows virtual key code (`windows_vk`); falls back to `key_name` if absent. If the user edits `key_name` through Settings after a Windows capture, `windows_vk` is cleared so the new `key_name` takes effect.

**Windows VK**
Windows virtual key code (`i32`) identifying the activation key on the Windows platform. Stored in config as `windows_vk`. Captured at runtime via a low-level keyboard hook (`WH_KEYBOARD_LL`) in capture mode. Preferred over `key_name` when running on Windows, unless it has just been cleared by a manual `key_name` edit.

**Windows VK Name Map**
A small mapping from X11-style keysym names to Windows virtual-key codes, used as a fallback when `windows_vk` is absent. Supports the keys most commonly chosen as the activation key: `Alt_L`, `Alt_R`, `Control_L`, `Control_R`, `Shift_L`, `Shift_R`, `space`, `Return`, `Tab`, `Escape`, and `F1`–`F12`. Unknown names cause `WindowsListener::new` to fail fast with a clear error, so users know immediately instead of wondering why the key does not trigger recording.

**Windows Capture Mode**
When the user presses a key during Windows activation-key capture, the low-level hook returns the Windows virtual-key code. The response stores it as `windows_vk` and presents an X11-style `key_name` (e.g. `Alt_R`) so the displayed activation key remains consistent across platforms. The capture implementation lives in `key_listener::windows` and is re-exported through `key_capture` as `CaptureActivationResponse`.

**State Machine**
The 5-state FSM (`Idle → PotentialPress → Recording → WaitSecondClick → ContinuousRecording`) that translates raw key events into `StartRecord` / `StopRecord` commands for the pipeline.
