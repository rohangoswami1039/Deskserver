# Phase 1 — Prove the Pipe

**Date:** 2026-05-31
**Status:** Approved
**Scope:** Phase 1 only — mouse move, clicks, and scroll forwarding over TCP

## Goal

Forward mouse events (movement, clicks, scroll) from a server machine to a client machine over TCP on a LAN. Both cursors will move (no suppression). This validates the cross-platform input pipe before investing in native hooks.

## Platforms

- macOS (server and client)
- Windows (server and client)
- All four combinations: Mac→Mac, Mac→Win, Win→Win, Win→Mac

## Project Structure

Cargo workspace with two binary crates and one shared library:

```
Deskserver/
├── Cargo.toml              # workspace root
├── crates/
│   ├── common/             # shared types + protocol
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── server/             # kvm-server binary
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── client/             # kvm-client binary
│       ├── Cargo.toml
│       └── src/main.rs
├── BUILDING_A_SOFTWARE_KVM.md
└── .gitignore
```

## Wire Protocol

### Message Types

```rust
#[derive(Serialize, Deserialize, Debug)]
enum InputMsg {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
}

#[derive(Serialize, Deserialize, Debug)]
enum MouseButton {
    Left,
    Right,
    Middle,
}
```

### Framing

- Each message serialized with `bincode`, prefixed with a `u32` little-endian length.
- `write_msg(stream, &msg)` — serialize, write 4-byte length + payload.
- `read_msg(stream) -> InputMsg` — read 4-byte length, read that many bytes, deserialize.
- Max frame size: 1 MiB. Reject anything larger.

## Server (`kvm-server`)

- **Args:** None. Binds `0.0.0.0:24800`, accepts one client. Sets `TCP_NODELAY`.
- **Two threads:**
  - Main thread: `rdev::listen` (must be main thread on macOS). Callback converts events to `InputMsg`, sends via `mpsc` channel.
  - Writer thread: receives from channel, calls `write_msg` to TCP stream.
- **Events forwarded:** `MouseMove`, `ButtonPress`, `ButtonRelease`, `Wheel`. All others ignored.
- **Lifecycle:** Prints IP/port on startup. Logs client connection. Exits on client disconnect.

## Client (`kvm-client`)

- **Args:** One positional — server IP (e.g., `192.168.1.50`). Connects to port `24800`. Sets `TCP_NODELAY`.
- **Single thread:** Blocking loop: `read_msg` → match → synthesize via `enigo`.
- **Synthesis:**
  - `MouseMove { x, y }` → `enigo.move_mouse(x as i32, y as i32, Coordinate::Abs)`
  - `MouseButton { button, pressed }` → `enigo.button(mapped_button, Press/Release)`
  - `Wheel { dx, dy }` → `enigo.scroll(dy, Vertical)` and `enigo.scroll(dx, Horizontal)`
- **Lifecycle:** Prints "connected" on success. Exits on disconnect/error.

## Dependencies

```toml
# common
serde = { version = "1", features = ["derive"] }
bincode = "1"

# server
rdev = "0.5"

# client
enigo = "0.2"
```

Versions pinned to match API signatures in the spec. Will bump if build issues arise.

## Platform Permissions

- **macOS server:** Input Monitoring (for `rdev` listen). Prompted on first run.
- **macOS client:** Accessibility (for `enigo` synthesis). Prompted on first run.
- **Windows:** No special permissions needed for Phase 1.

## Out of Scope

- Input suppression (Phase 2)
- Keyboard forwarding (Phase 2)
- Edge transition / screen layout (Phase 3)
- Clipboard, encryption, discovery, UI (Phase 4)
- Reconnection logic
- Multi-client support

## Success Criteria

1. Run `cargo run --release -p kvm-server` on machine A.
2. Run `cargo run --release -p kvm-client <server-ip>` on machine B.
3. Move mouse on A → cursor on B mirrors the position.
4. Click on A → click synthesized on B.
5. Scroll on A → scroll synthesized on B.
6. Works for all four platform combinations (Mac↔Mac, Mac↔Win).
