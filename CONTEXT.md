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
The trait that receives events from the running pipeline: status changes, progress, errors, and transcription results. `TauriPipelineSink` in `cmd.rs` is the single concrete adapter in production use.

## Configuration

**ConfigStore**
Holds the live in-memory config behind a `Mutex` alongside the backing file path. Exposes `snapshot`, `snapshot_blocking`, and `apply_patch`. All config mutations go through `apply_patch`, which validates and persists atomically. Lives in `config_store.rs`.

**ConfigPatch**
A partial update to the config: all fields optional, absent fields left unchanged. The `linux_evdev_code` field uses a three-state deserialiser to distinguish absent (no change) from JSON `null` (clear the stored code). This is the type accepted by `save_config` over IPC.

## History

**HistoryStore**
Wraps the history JSON file and exposes named operations: `list`, `append`, `delete`, `clear`, `get`, `update_text`. Callers never handle the file path. Lives in `history.rs`. Each instance is cheap to clone (it contains only a `PathBuf`).

**HistoryEntry**
A single transcription record: `id`, `createdAtMs`, `rawText` (Whisper output), `text` (polished or same as raw). Audio is never stored.

## Output

**Overlay**
The floating status window shown during recording, processing, and result display. Positioned on the primary monitor via `xrandr` geometry. Managed directly by `TauriPipelineSink` on status transitions.

**Polisher**
The optional LLM post-processing step. Controlled by `PolishLevel` (`none`/`light`/`medium`/`heavy`). Communicates with any OpenAI-compatible chat API.

## Key Input

**Activation Key**
The physical key held to start recording. Configured per-device as either an X11 keysym name (`key_name`) or an evdev scancode (`linux_evdev_code`). The evdev path is preferred on Wayland.

**State Machine**
The 5-state FSM (`Idle → PotentialPress → Recording → WaitSecondClick → ContinuousRecording`) that translates raw key events into `StartRecord` / `StopRecord` commands for the pipeline.
