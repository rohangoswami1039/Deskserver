# Phase 1 — Prove the Pipe: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forward mouse events (move, click, scroll) from a server to a client over TCP on a LAN using `rdev` + `enigo`.

**Architecture:** Cargo workspace with three crates — `common` (wire protocol), `server` (capture via `rdev`, send over TCP), `client` (receive, synthesize via `enigo`). Server uses two threads (main for `rdev::listen`, spawned for TCP writes). Client runs a single-threaded blocking read loop.

**Tech Stack:** Rust, serde + bincode (serialization), rdev (event capture), enigo (event synthesis), std::net (TCP)

---

## File Structure

```
Deskserver/
├── Cargo.toml                    # workspace root
├── .gitignore                    # Rust gitignore
├── crates/
│   ├── common/
│   │   ├── Cargo.toml            # serde, bincode deps
│   │   └── src/
│   │       └── lib.rs            # InputMsg, MouseButton, frame read/write
│   ├── server/
│   │   ├── Cargo.toml            # depends on common, rdev
│   │   └── src/
│   │       └── main.rs           # TcpListener, rdev::listen, mpsc → write loop
│   └── client/
│       ├── Cargo.toml            # depends on common, enigo
│       └── src/
│           └── main.rs           # TcpStream connect, read loop → enigo synthesis
└── tests/
    └── integration/
        └── protocol_test.rs      # round-trip serialization tests
```

---

### Task 1: Scaffold the Cargo Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `.gitignore`
- Create: `crates/common/Cargo.toml`
- Create: `crates/common/src/lib.rs` (empty)
- Create: `crates/server/Cargo.toml`
- Create: `crates/server/src/main.rs` (stub)
- Create: `crates/client/Cargo.toml`
- Create: `crates/client/src/main.rs` (stub)

- [ ] **Step 1: Create workspace root `Cargo.toml`**

```toml
[workspace]
members = ["crates/common", "crates/server", "crates/client"]
resolver = "2"
```

- [ ] **Step 2: Create `.gitignore`**

```
/target
Cargo.lock
```

- [ ] **Step 3: Create `crates/common/Cargo.toml`**

```toml
[package]
name = "deskserver-common"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
bincode = "1"
```

- [ ] **Step 4: Create `crates/common/src/lib.rs`**

```rust
// Wire protocol types — populated in Task 2
```

- [ ] **Step 5: Create `crates/server/Cargo.toml`**

```toml
[package]
name = "kvm-server"
version = "0.1.0"
edition = "2021"

[dependencies]
deskserver-common = { path = "../common" }
rdev = "0.5"
```

- [ ] **Step 6: Create `crates/server/src/main.rs`**

```rust
fn main() {
    println!("kvm-server starting...");
}
```

- [ ] **Step 7: Create `crates/client/Cargo.toml`**

```toml
[package]
name = "kvm-client"
version = "0.1.0"
edition = "2021"

[dependencies]
deskserver-common = { path = "../common" }
enigo = "0.2"
```

- [ ] **Step 8: Create `crates/client/src/main.rs`**

```rust
fn main() {
    println!("kvm-client starting...");
}
```

- [ ] **Step 9: Verify the workspace builds**

Run: `cargo build`
Expected: All three crates compile successfully with no errors.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml .gitignore crates/
git commit -m "feat: scaffold Cargo workspace with common, server, client crates"
```

---

### Task 2: Implement the Wire Protocol (common crate)

**Files:**
- Modify: `crates/common/src/lib.rs`
- Create: `tests/integration/protocol_test.rs`
- Modify: `Cargo.toml` (workspace root — add test)

- [ ] **Step 1: Write the failing serialization round-trip test**

Create `tests/integration/protocol_test.rs`:

```rust
use deskserver_common::{InputMsg, MouseButton, read_msg, write_msg};
use std::io::Cursor;

#[test]
fn roundtrip_mouse_move() {
    let msg = InputMsg::MouseMove { x: 100.5, y: 200.75 };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();

    match decoded {
        InputMsg::MouseMove { x, y } => {
            assert!((x - 100.5).abs() < f64::EPSILON);
            assert!((y - 200.75).abs() < f64::EPSILON);
        }
        _ => panic!("expected MouseMove, got {:?}", decoded),
    }
}

#[test]
fn roundtrip_mouse_button_press() {
    let msg = InputMsg::MouseButton {
        button: MouseButton::Left,
        pressed: true,
    };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();

    match decoded {
        InputMsg::MouseButton { button, pressed } => {
            assert!(matches!(button, MouseButton::Left));
            assert!(pressed);
        }
        _ => panic!("expected MouseButton, got {:?}", decoded),
    }
}

#[test]
fn roundtrip_mouse_button_release() {
    let msg = InputMsg::MouseButton {
        button: MouseButton::Right,
        pressed: false,
    };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();

    match decoded {
        InputMsg::MouseButton { button, pressed } => {
            assert!(matches!(button, MouseButton::Right));
            assert!(!pressed);
        }
        _ => panic!("expected MouseButton, got {:?}", decoded),
    }
}

#[test]
fn roundtrip_wheel() {
    let msg = InputMsg::Wheel { dx: -3, dy: 5 };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();

    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();

    match decoded {
        InputMsg::Wheel { dx, dy } => {
            assert_eq!(dx, -3);
            assert_eq!(dy, 5);
        }
        _ => panic!("expected Wheel, got {:?}", decoded),
    }
}

#[test]
fn reject_oversized_frame() {
    // Craft a frame with length header claiming 2 MiB (over 1 MiB limit)
    let fake_len: u32 = 2 * 1024 * 1024;
    let mut buf = Vec::new();
    buf.extend_from_slice(&fake_len.to_le_bytes());
    buf.extend_from_slice(&[0u8; 64]); // partial garbage payload

    let mut cursor = Cursor::new(&buf);
    let result = read_msg(&mut cursor);
    assert!(result.is_err());
}

#[test]
fn multiple_messages_in_sequence() {
    let msgs = vec![
        InputMsg::MouseMove { x: 1.0, y: 2.0 },
        InputMsg::MouseButton { button: MouseButton::Middle, pressed: true },
        InputMsg::Wheel { dx: 0, dy: -1 },
        InputMsg::MouseButton { button: MouseButton::Middle, pressed: false },
    ];

    let mut buf: Vec<u8> = Vec::new();
    for msg in &msgs {
        write_msg(&mut buf, msg).unwrap();
    }

    let mut cursor = Cursor::new(&buf);
    for expected in &msgs {
        let decoded = read_msg(&mut cursor).unwrap();
        assert_eq!(format!("{:?}", decoded), format!("{:?}", expected));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test protocol_test`
Expected: FAIL — `InputMsg`, `MouseButton`, `read_msg`, `write_msg` not found.

- [ ] **Step 3: Implement the wire protocol in `crates/common/src/lib.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

const MAX_FRAME_SIZE: u32 = 1024 * 1024; // 1 MiB

#[derive(Serialize, Deserialize, Debug)]
pub enum InputMsg {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Write a length-prefixed bincode frame to the writer.
pub fn write_msg<W: Write>(writer: &mut W, msg: &InputMsg) -> io::Result<()> {
    let payload = bincode::serialize(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed bincode frame from the reader.
pub fn read_msg<R: Read>(reader: &mut R) -> io::Result<InputMsg> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf);

    if len > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes (max {})", len, MAX_FRAME_SIZE),
        ));
    }

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    bincode::deserialize(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test protocol_test`
Expected: All 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/lib.rs tests/
git commit -m "feat: implement wire protocol with length-prefixed bincode framing"
```

---

### Task 3: Implement the Server

**Files:**
- Modify: `crates/server/src/main.rs`

- [ ] **Step 1: Implement the server**

Replace `crates/server/src/main.rs`:

```rust
use deskserver_common::{write_msg, InputMsg, MouseButton};
use rdev::{listen, EventType, Button};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

const PORT: u16 = 24800;

fn map_button(b: Button) -> Option<MouseButton> {
    match b {
        Button::Left => Some(MouseButton::Left),
        Button::Right => Some(MouseButton::Right),
        Button::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

fn main() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .expect("failed to bind");

    // Print all local IPs so the user knows what to connect to
    println!("kvm-server listening on 0.0.0.0:{}", PORT);
    println!("waiting for client connection...");

    let (mut stream, addr) = listener.accept().expect("failed to accept client");
    stream.set_nodelay(true).expect("failed to set TCP_NODELAY");
    println!("client connected: {}", addr);

    let (tx, rx) = mpsc::channel::<InputMsg>();

    // Writer thread: drain channel → TCP
    thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(msg) => {
                    if let Err(e) = write_msg(&mut stream, &msg) {
                        eprintln!("write error: {}", e);
                        break;
                    }
                }
                Err(_) => {
                    // Sender dropped — listener stopped
                    break;
                }
            }
        }
        println!("writer thread exiting");
    });

    // Main thread: rdev::listen (must be main thread on macOS)
    println!("capturing mouse events... (Ctrl+C to stop)");
    listen(move |event| {
        let msg = match event.event_type {
            EventType::MouseMove { x, y } => {
                Some(InputMsg::MouseMove { x, y })
            }
            EventType::ButtonPress(b) => {
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: true,
                })
            }
            EventType::ButtonRelease(b) => {
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: false,
                })
            }
            EventType::Wheel { delta_x, delta_y } => {
                Some(InputMsg::Wheel {
                    dx: delta_x,
                    dy: delta_y,
                })
            }
            _ => None,
        };

        if let Some(msg) = msg {
            let _ = tx.send(msg);
        }
    })
    .expect("failed to start event listener");
}
```

- [ ] **Step 2: Verify the server compiles**

Run: `cargo build -p kvm-server`
Expected: Compiles with no errors. (Cannot unit-test `rdev::listen` — it requires a real display server.)

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat: implement kvm-server with rdev capture and TCP forwarding"
```

---

### Task 4: Implement the Client

**Files:**
- Modify: `crates/client/src/main.rs`

- [ ] **Step 1: Implement the client**

Replace `crates/client/src/main.rs`:

```rust
use deskserver_common::{read_msg, InputMsg, MouseButton as ProtoButton};
use enigo::{Enigo, MouseButton, MouseControllable};
use std::env;
use std::net::TcpStream;

const PORT: u16 = 24800;

fn map_button(b: &ProtoButton) -> MouseButton {
    match b {
        ProtoButton::Left => MouseButton::Left,
        ProtoButton::Right => MouseButton::Right,
        ProtoButton::Middle => MouseButton::Middle,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: kvm-client <server-ip>");
        std::process::exit(1);
    }
    let server_ip = &args[1];

    let addr = format!("{}:{}", server_ip, PORT);
    println!("connecting to {}...", addr);

    let mut stream = TcpStream::connect(&addr).expect("failed to connect to server");
    stream.set_nodelay(true).expect("failed to set TCP_NODELAY");
    println!("connected to server at {}", addr);

    let mut enigo = Enigo::new();

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => match msg {
                InputMsg::MouseMove { x, y } => {
                    enigo.mouse_move_to(x as i32, y as i32);
                }
                InputMsg::MouseButton { button, pressed } => {
                    let btn = map_button(&button);
                    if pressed {
                        enigo.mouse_down(btn);
                    } else {
                        enigo.mouse_up(btn);
                    }
                }
                InputMsg::Wheel { dx: _, dy } => {
                    enigo.mouse_scroll_y(dy as i32);
                }
            },
            Err(e) => {
                eprintln!("read error (server disconnected?): {}", e);
                break;
            }
        }
    }

    println!("client exiting");
}
```

- [ ] **Step 2: Verify the client compiles**

Run: `cargo build -p kvm-client`
Expected: Compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/client/src/main.rs
git commit -m "feat: implement kvm-client with enigo synthesis"
```

---

### Task 5: Build, Test, and Verify End-to-End

**Files:** None new — this is a verification task.

- [ ] **Step 1: Run all unit/integration tests**

Run: `cargo test`
Expected: All protocol round-trip tests pass. Server and client stubs compile.

- [ ] **Step 2: Build release binaries**

Run: `cargo build --release`
Expected: Binaries at `target/release/kvm-server` and `target/release/kvm-client`.

- [ ] **Step 3: Verify server starts**

Run: `cargo run --release -p kvm-server`
Expected output:
```
kvm-server listening on 0.0.0.0:24800
waiting for client connection...
```
(Ctrl+C to stop after verifying.)

- [ ] **Step 4: Verify client shows usage on no args**

Run: `cargo run --release -p kvm-client`
Expected output:
```
usage: kvm-client <server-ip>
```

- [ ] **Step 5: Commit any final fixes if needed**

If any compilation or test issues were fixed during verification:

```bash
git add -A
git commit -m "fix: resolve build/test issues from end-to-end verification"
```

---

## Verification Checklist

- [ ] `cargo test` — all protocol tests pass
- [ ] `cargo build --release` — both binaries compile for the current platform
- [ ] `kvm-server` starts and listens on port 24800
- [ ] `kvm-client` prints usage when no args given
- [ ] `kvm-client <ip>` connects to a running server
- [ ] Mouse movement on server → cursor moves on client
- [ ] Mouse clicks on server → clicks synthesized on client
- [ ] Mouse scroll on server → scroll synthesized on client

> **Note:** The last four items require two machines on the same LAN (or a VM). They cannot be automated in CI.
