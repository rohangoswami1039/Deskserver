# Phase 2 — Native Capture, Suppression & Keyboard

**Date:** 2026-06-01
**Status:** Approved
**Scope:** Replace rdev with native hooks, add hotkey toggle (Ctrl+Shift+Space), input suppression, keyboard forwarding with Cmd↔Ctrl swap

## Goal

When the user presses Ctrl+Shift+Space, the server switches to REMOTE mode: local mouse and keyboard events are suppressed (cursor freezes) and forwarded to the client. Pressing the hotkey again switches back to LOCAL mode. Keyboard events are forwarded with automatic Cmd↔Ctrl mapping between macOS and Windows.

## Platforms

- macOS server (CGEventTap)
- Windows server (SetWindowsHookEx)
- macOS and Windows client (enigo synthesis — already works for mouse, adding keyboard)

## State Machine

```
LOCAL ←→ REMOTE  (toggled by Ctrl+Shift+Space)
```

- **LOCAL:** All input passes through to the server's OS. Nothing forwarded to client.
- **REMOTE:** All mouse and keyboard events are suppressed locally and forwarded to the client. Cursor is hidden and warped to screen center.

State stored in `AtomicU8` for lock-free access from the capture callback.

On switch to REMOTE:
- Warp cursor to screen center
- Hide cursor (macOS: `CGDisplayHideCursor`, Windows: `ShowCursor(FALSE)`)
- Send `ScreenEnter` to client

On switch to LOCAL:
- Show cursor
- Send `ScreenLeave` to client

## Wire Protocol Changes

```rust
enum InputMsg {
    // Existing
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },

    // New
    KeyDown { key: u32, modifiers: u8 },
    KeyUp { key: u32, modifiers: u8 },
    ScreenEnter,
    ScreenLeave,
}
```

- `key`: neutral key ID (not raw OS keycode). Mapped via keymap tables.
- `modifiers`: bitfield — `Shift=0x01, Ctrl=0x02, Alt=0x04, Meta=0x08`.
- Meta = Cmd on macOS, Win key on Windows.
- Cmd↔Ctrl swap applied at sender side before serialization.

## Server Architecture

```
crates/server/src/
├── main.rs              # entry point, state machine, TCP, hotkey detection
├── capture/
│   ├── mod.rs           # CaptureEvent enum, run_capture() dispatcher
│   ├── macos.rs         # CGEventTap implementation
│   └── windows.rs       # SetWindowsHookEx implementation
└── keymap.rs            # neutral key ID ↔ OS keycode tables, Cmd↔Ctrl swap
```

### CaptureEvent (internal)

```rust
enum CaptureEvent {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
    KeyDown { keycode: u32, modifiers: u8 },
    KeyUp { keycode: u32, modifiers: u8 },
}
```

### run_capture()

- Signature: takes `FnMut(CaptureEvent) -> bool` — return `true` to suppress, `false` to pass through.
- Must be called on the main thread.
- macOS: creates CGEventTap, runs CFRunLoop.
- Windows: installs WH_MOUSE_LL + WH_KEYBOARD_LL hooks, runs message loop.

### main.rs flow

1. Bind TCP on port 24800, accept client, set TCP_NODELAY.
2. Call `run_capture()` with a callback that:
   - Detects Ctrl+Shift+Space → toggles InputMode
   - When REMOTE: convert CaptureEvent → InputMsg (applying keymap + Cmd↔Ctrl swap), write to TCP via Mutex<TcpStream>, return `true` (suppress)
   - When LOCAL: return `false` (pass through)

## Platform Details

### macOS (macos.rs)

- `CGEventTap` with `.defaultTap` + `.headInsertEventTap`
- Requires **Accessibility** permission
- Callback returns `nil` to suppress, returns event to pass through
- Handle `kCGEventTapDisabledByTimeout` — re-enable tap
- Event mask: mouse move, mouse buttons, scroll, key down, key up, flags changed
- FFI via `core-graphics` and `core-foundation` crates

### Windows (windows.rs)

- `SetWindowsHookEx` with `WH_MOUSE_LL` and `WH_KEYBOARD_LL`
- Callback must return within 1000ms — only check state and suppress, no I/O
- Suppress: return non-zero without calling `CallNextHookEx`
- Pass through: call `CallNextHookEx`
- Hook thread runs message loop (`GetMessage` + `TranslateMessage` + `DispatchMessage`)
- Events sent to main thread via channel for TCP writes

### Cursor hide/show

- macOS: `CGDisplayHideCursor(CGMainDisplayID())` / `CGDisplayShowCursor(...)`
- Windows: `ShowCursor(FALSE)` / `ShowCursor(TRUE)`

## Keymap (keymap.rs)

- `HashMap<u32, u32>` mapping OS keycode → neutral ID (and reverse)
- Covers ~80 common keys: A-Z, 0-9, F1-F12, arrows, modifiers, Enter, Tab, Escape, Backspace, Delete, Space, common punctuation
- macOS uses `CGKeyCode` values (e.g., 0=A, 13=W)
- Windows uses virtual key codes (e.g., 0x41=A, 0x57=W)
- Cmd↔Ctrl swap: when sending from Mac, `Meta` bit → `Ctrl` bit. When sending from Windows, `Ctrl` bit → `Meta` bit. Applied before serialization.

## Client Changes

- Handle `KeyDown { key, modifiers }`: map neutral ID → local OS keycode, synthesize with enigo
- Handle `KeyUp { key, modifiers }`: same, release
- Handle `ScreenEnter`: log "now controlling this machine"
- Handle `ScreenLeave`: log "control returned to server"
- Architecture unchanged: single-threaded blocking read loop

## Dependencies

### Server
- Remove: `rdev`
- Add (macOS): `core-graphics = "0.23"`, `core-foundation = "0.9"`
- Add (Windows): `windows = "0.58"` with features for hooks, input, cursor APIs

### Client
- No new dependencies. `enigo 0.2.1` handles keyboard synthesis.

## Firewall Note

Server binary stays named `test_server` on Mac to keep the existing macOS firewall allow rule. The `default-run = "test_server"` in server Cargo.toml is preserved.

## Out of Scope

- Edge transition (Phase 3)
- Screen layout graph (Phase 3)
- DPI/scaling (Phase 3)
- Clipboard, encryption, discovery, UI (Phase 4)
- Reconnection / multi-client
- IME, dead keys, AltGr, non-US layouts (deferred)

## Success Criteria

1. Press Ctrl+Shift+Space on the server → cursor freezes, "REMOTE" mode logged.
2. Move mouse on server → only the client cursor moves.
3. Click on server → click synthesized on client only.
4. Type on server keyboard → keystrokes appear on client only.
5. Cmd+C on Mac server → Ctrl+C received on Windows client (and vice versa).
6. Press Ctrl+Shift+Space again → cursor unfreezes, local control restored.
7. Works with macOS server and Windows server.
