# ADR-0002: Windows Key Listener via WH_KEYBOARD_LL Message-Pump Thread

**Date**: 2026-06-13
**Status**: Accepted
**Related**: ADR-0001, issue #19

## Context

With ADR-0001 we committed to restoring Windows as a first-class platform using native Win32 APIs. The voice pipeline needs a global activation key listener on Windows. The previous Windows implementation used PowerShell subprocesses, which were unreliable and had injection risks. We need a native mechanism that can listen for global key events without requiring a window.

## Decision

Implement the Windows key listener with a `WH_KEYBOARD_LL` low-level keyboard hook running in a dedicated message-pump thread.

### Why WH_KEYBOARD_LL

- Captures global key events without an owned window.
- Runs in-process, avoiding subprocess overhead and injection risks.
- Callback executes in the thread that installed the hook, making event forwarding straightforward.

### Message-Pump Thread Design

- `WindowsListener::start()` spawns a background thread and returns `(UnboundedReceiver<KeyEvent>, BACKEND_NAME)` where `BACKEND_NAME` is the constant `"wh_keyboard_ll"`.
- The thread installs the hook, then runs a `GetMessageW` / `TranslateMessage` / `DispatchMessageW` loop.
- `Drop` for `WindowsListener` sets an `AtomicBool` stopping flag, sends `WM_QUIT` to the thread via `PostThreadMessageW`, waits for it to join, and then calls `UnhookWindowsHookEx` from inside the exiting thread to avoid cross-thread hook uninstallation.
- The hook callback filters by the configured virtual-key code and sends `KeyEvent { pressed: bool }` through a `tokio::sync::mpsc::UnboundedSender` to the async pipeline.

### VK Resolution

- Primary source is `windows_vk` from `KeyListenerConfig`.
- If `windows_vk` is `None`, resolve `key_name` through `vk_from_key_name`, a small X11-style keysym-to-VK mapping table (`Alt_L`, `Alt_R`, `Control_L`, `Control_R`, `Shift_L`, `Shift_R`, `space`, `Return`, `Tab`, `Escape`, `F1`–`F12`).
- Capture mode uses `key_name_from_vk` to convert a captured VK code back to an X11-style display name. Extended left/right variants (e.g. `VK_LSHIFT` / `VK_RSHIFT`) map to `Shift_L` / `Shift_R`; non-extended variants map to a reasonable canonical name.
- Unknown `key_name` values fail fast at listener construction time.

### Activation-Key Capture

- Capture mode installs the same `WH_KEYBOARD_LL` hook temporarily, waits up to 12 seconds for the first `WM_KEYDOWN`, records the VK code, then uninstalls. If no key is pressed within the timeout, it returns an error matching the Linux capture behavior.
- The returned `CaptureActivationResponse` stores the VK code in `windows_vk` and uses an X11-style `key_name` so the displayed key stays consistent across platforms.

## Consequences

- Requires the `windows` crate (`0.61`) as a Windows-only dependency.
- The listener owns a background thread and must be stopped cleanly on `Drop` to avoid leaked hooks or threads.
- Event forwarding is thread-safe because `UnboundedSender` is `Send + Sync`.
- Unknown `key_name` values fail at startup rather than silently ignoring keys.
- Unit tests cover the VK name/code mappings (round-trip), listener construction with valid/invalid configs, and `KeyEvent` construction. Hook lifecycle is not unit-tested because it requires a real Windows desktop session.

## Alternatives Considered

- **Raw Input (`RegisterRawInputDevices`)**: Requires a window handle to receive `WM_INPUT`, which conflicts with altgo's windowless background pipeline.
- **`GetAsyncKeyState` polling**: Wastes CPU and misses brief key presses.
- **Tauri global shortcut plugin**: Insufficient control over hold-to-record semantics and does not expose key-up events for long-press detection.
