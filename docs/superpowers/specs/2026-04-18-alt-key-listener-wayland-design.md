# Alt Key Listener Wayland Support — Design

## Status

Approved for implementation.

## Background

The current Linux key listener uses `xinput test-xi2` on X11 and falls back to `evtest` reading `/dev/input/event*` on Wayland. However, GNOME Shell (and other Wayland compositors) intercept all keyboard events before they reach `/dev/input/event*`, making the evtest fallback non-functional on Wayland sessions.

Goal: implement reliable global Alt key capture on Wayland (GNOME Shell) via a GNOME Shell extension, with graceful fallback on other platforms/sessions.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  GNOME Shell (Compositor)                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  GNOME Shell Extension (JavaScript)                  │   │
│  │  - Intercepts Alt key events via MetaKeyManager      │   │
│  │  - Sends events over Unix socket to altgo            │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            ↕ Unix socket (127.0.0.1:19623)
┌─────────────────────────────────────────────────────────────┐
│  altgo (Rust/Tauri)                                         │
│  - Detects Wayland on startup                               │
│  - Attempts GNOME extension socket first                     │
│  - Falls back to X11/evtest if extension unavailable         │
└─────────────────────────────────────────────────────────────┘
```

---

## Components

### 1. GNOME Shell Extension

Location: `extensions/altgo-key-listener/`

```
extensions/altgo-key-listener/
├── extension.js      # Main extension code
├── metadata.json      # GNOME extension metadata
└── README.md         # Installation instructions
```

**Behavior:**
- On `enable()`: connect to `127.0.0.1:19623`
- Register global key press/release listener via `Meta.keyval_bindings`
- Listen for RightAlt (or LeftAlt, configurable) keycode `65515` / `65516`
- On key press: send `P\n` over socket
- On key release: send `R\n` over socket
- If socket connection fails, extension logs warning and continues silently
- On `disable()`: close socket, unregister bindings

**Configuration:**
- Default: RightAlt only
- Future: configurable via GSettings (scope deferred to post-MVP)

### 2. Rust Socket Client

New module: `src-tauri/src/key_listener/gnome_extension.rs`

**Behavior:**
- `GnomeExtensionListener::new()` — creates TCP socket client to `127.0.0.1:19623`
- `start()` — spawns async task that reads `P\n` / `R\n` lines and converts to `KeyEvent`
- If connection fails on first attempt, retries every 5 seconds with logging
- Returns `mpsc::UnboundedReceiver<KeyEvent>` like other listeners
- `stop()` — closes socket, signals thread exit

### 3. Listener Selection Logic

Modified `src-tauri/src/key_listener/mod.rs`:

Detection order:
1. If `XDG_SESSION_TYPE=wayland` and `GNOME_DESKTOP_SESSION_ID` set → try `GnomeExtensionListener` first
2. If X11 (`DISPLAY` set, `XDG_SESSION_TYPE != wayland`) → try `X11Listener`
3. If Wayland without extension → try `EvtestListener` (will fail with clear error)
4. If no listener available → return error with actionable message

### 4. Platform Listener Type Alias

```rust
#[cfg(target_os = "linux")]
pub type PlatformListener = CompositeListener;
```

Where `CompositeListener` tries listeners in priority order.

---

## IPC Protocol

Simple line-oriented text protocol over TCP:

```
P\n   <- key pressed
R\n   <- key released
```

No heartbeat. No ACK. Extension sends events fire-and-forget.

---

## Fallback Behavior

| Session | Extension | Fallback |
|---------|-----------|----------|
| X11 | N/A | X11Listener (xinput) |
| Wayland + GNOME + Extension | Works | None needed |
| Wayland + GNOME (no extension) | Not available | EvtestListener → clear error |
| Wayland + other compositor | N/A | EvtestListener → clear error |

---

## Error Messages

When key listener fails on Wayland without extension:

> "Cannot capture Alt key on Wayland. Install the GNOME Shell extension to enable global key capture on Wayland."
>
> Run: `make install-extension`

---

## File Changes

```
src-tauri/src/key_listener/
  gnome_extension.rs     # NEW — socket client
  mod.rs               # MOD — add CompositeListener, update detection

extensions/
  altgo-key-listener/  # NEW — GNOME Shell extension
    extension.js
    metadata.json
    README.md

Makefile               # MOD — add install-extension target
```

---

## Testing Plan

1. **Unit tests** — `gnome_extension.rs` mock socket tests
2. **Integration** — Run `cargo tauri dev` on X11 and Wayland, verify listener selection
3. **Manual** — Install extension, verify Alt key events appear in debug logs

---

## Out of Scope (Post-MVP)

- Configurable key (LeftAlt vs RightAlt) — hardcoded to RightAlt for now
- GSettings-based configuration UI in GNOME
- Non-GNOME Wayland support (KDE Plasma, Sway, etc.)
- macOS accessibility API equivalent
