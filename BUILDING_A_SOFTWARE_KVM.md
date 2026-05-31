# Building a Cross-Platform Software KVM from Scratch

A complete engineering guide to building software that shares **one keyboard and
mouse** across Mac and Windows machines on the same Wi-Fi/LAN. Move the cursor to
the edge of one screen and control "jumps" to the next computer, with the keyboard
following the active machine.

This is *not* screen sharing. There is no video, no virtual display driver, no
streaming. We are only forwarding input events.

> **Scope:** Mac→Mac, Mac→Windows, Windows→Windows (and the reverse). Same user,
> same LAN.

---

## Table of contents

1. [How a software KVM works](#1-how-a-software-kvm-works)
2. [Tech stack](#2-tech-stack)
3. [Architecture](#3-architecture)
4. [The wire protocol](#4-the-wire-protocol)
5. [Phase 1 — Prove the pipe](#5-phase-1--prove-the-pipe)
6. [Phase 2 — Native capture, suppression & keyboard](#6-phase-2--native-capture-suppression--keyboard)
7. [Phase 3 — Edge transition & screen layout](#7-phase-3--edge-transition--screen-layout)
8. [Phase 4 — Clipboard, encryption, discovery, UX](#8-phase-4--clipboard-encryption-discovery-ux)
9. [OS permissions & hard limits](#9-os-permissions--hard-limits)
10. [Security checklist](#10-security-checklist)
11. [Testing & latency tuning](#11-testing--latency-tuning)
12. [Packaging & distribution](#12-packaging--distribution)
13. [Reference projects to study](#13-reference-projects-to-study)

---

## 1. How a software KVM works

One machine is the **server** — the one with the physical keyboard and mouse
attached. Every other machine is a **client**. The server:

1. **Captures** global input events (mouse motion, clicks, scroll, keystrokes).
2. **Decides** which screen currently "owns" the cursor, using a configured map of
   how the screens are arranged (e.g. "Windows-PC is to the right of the MacBook").
3. While the cursor is on a **remote** screen, it **suppresses** the local effect of
   each event (so the server's own machine doesn't react) and **forwards** the event
   over the network.
4. The target **client** receives the event and **synthesizes** it locally, as if it
   came from real hardware.

The cursor "crosses" between machines when it hits a screen edge that is linked to a
neighbour in the layout. The protocol is OS-neutral, so any machine can be server or
client regardless of platform.

**What's easy vs. hard:**

| Part | Difficulty | Why |
|------|-----------|-----|
| Networking the events | Easy | A few hundred bytes/sec over LAN; sub-ms RTT. |
| Synthesizing events on the client | Medium | Mature OS APIs exist (`SendInput`, `CGEventPost`). |
| Capturing global input | Medium | OS-specific hooks, permission-gated. |
| **Suppressing local input while remote** | **Hard** | Must swallow events without locking the machine. |
| **Cross-OS keyboard mapping** | **Hard** | Cmd vs Ctrl, layouts, dead keys, IME. |
| **OS permissions / signing** | **Hard** | macOS TCC, Windows UAC/secure desktop. |

Plan your time around the bottom three rows. The networking is the part that
"just works."

---

## 2. Tech stack

**Recommended language: Rust.** Reasons:

- It is a security-sensitive app (you transmit keystrokes, possibly passwords);
  memory safety matters.
- Two mature crates cover the cross-platform input layer so you don't write FFI on
  day one:
  - **`enigo`** (MIT) — input **synthesis** (wraps `SendInput` on Windows, `CGEvent`
    on macOS, X11/Wayland on Linux).
  - **`rdev`** — global event **listening** (good enough for early phases; cannot
    *block* events, so you replace it with native hooks in Phase 2).
- Good ecosystem for the rest: `tokio` (async networking), `rustls` or `snow`
  (encryption), `mdns-sd` (discovery), `serde` + `bincode` (serialization),
  `egui`/`Tauri` (tray/config UI).

**Alternatives:**

- **C++ / Qt** — pick this only if you want Qt for the UI and don't mind manual
  memory management. (This is what the Synergy/Deskflow family uses.)
- **Swift / C#** — fine if you were committing to a single OS, which you are not.
- **Go** — solid for the daemon/networking, but weak for the low-level input layer
  (you'd write cgo FFI to the same C APIs anyway). Not recommended as the primary
  language.

**Crate summary (Rust path):**

```
serde + bincode   serialization / wire framing
rdev              Phase 1 capture (listen only)
enigo             event synthesis on the client
tokio             async networking (Phase 4)
rustls / snow     TLS or Noise encryption (Phase 4)
argon2 + chacha20poly1305   PIN-based pairing (Phase 4)
mdns-sd           zero-config discovery (Phase 4)
egui / tauri      system tray + layout editor UI (Phase 4)
```

---

## 3. Architecture

Four layers, present on every machine (a machine can act as server or client):

```
        SERVER (has the real keyboard/mouse)             CLIENT
   ┌───────────────────────────────────────┐     ┌────────────────────────┐
   │ 1. CAPTURE                              │     │ 4. SYNTHESIS            │
   │    native global hook                   │     │    enigo / SendInput /  │
   │    (CGEventTap / SetWindowsHookEx)       │     │    CGEventPost          │
   │                 │                        │     │            ▲           │
   │                 ▼                        │     │            │           │
   │ 2. DECIDE                                │     │            │           │
   │    which screen owns the cursor?         │     │            │           │
   │    if remote → suppress local event      │     │            │           │
   │                 │                        │     │            │           │
   │                 ▼                        │     │            │           │
   │ 3. TRANSPORT  ──────── TCP 24800 (TLS) ──┼─────┼──► read & decode frame  │
   │    serialize + length-prefix frame       │     │                        │
   └───────────────────────────────────────┘     └────────────────────────┘
```

- **Capture (server):** intercept input before the OS delivers it elsewhere.
- **Decide (server):** maintain the screen-layout graph and the "active screen"
  state; when the cursor is remote, swallow the local event and route it instead.
- **Transport:** length-prefixed binary frames over **TCP** with `TCP_NODELAY`.
  TCP (not UDP) because input events must arrive **in order and reliably** — a
  dropped key-up or reordered button event leaves stuck modifiers/buttons. The
  Synergy/Deskflow family uses TCP on port **24800**.
- **Synthesis (client):** inject the event using the OS API.

---

## 4. The wire protocol

Keep it small and tagged. Each frame is:

```
[ u32 little-endian length ][ payload bytes ]
```

The payload is a serialized message enum. Start with mouse-only and grow it:

```rust
enum InputMsg {
    MouseMove   { x: f64, y: f64 },        // Phase 1: absolute; Phase 3: deltas
    MouseButton { button: MouseButton, pressed: bool },
    Wheel       { dx: i64, dy: i64 },
    KeyDown     { key: u32, mods: u8 },    // Phase 2
    KeyUp       { key: u32, mods: u8 },    // Phase 2
    ScreenEnter,                           // Phase 3: cursor entered this client
    ScreenLeave,                           // Phase 3: cursor left this client
    Clipboard   { data: ClipboardData },   // Phase 4
    Heartbeat,                             // Phase 4: keepalive
}
```

Design rules:

- **Length-prefix every frame** so the reader always knows where a message ends.
- **Cap payload size** (e.g. reject frames larger than 32 MiB) to defend against a
  malicious or corrupt peer.
- **Send a `Heartbeat`** every few seconds; disconnect a peer that misses ~3.
- **Use a neutral key ID** in `KeyDown`/`KeyUp` (not a raw OS keycode) and let each
  client map it to its local layout. This is what makes cross-OS mapping tractable.
- Historically Synergy used readable 4-letter codes (`DMMV` = mouse move, `DKDN` =
  key down) sent in **plaintext** — which let anyone on the network sniff keystrokes
  off port 24800. Do not ship plaintext (see [Security](#10-security-checklist)).

---

## 5. Phase 1 — Prove the pipe

**Goal:** forward mouse events from server to client over TCP and watch the client's
cursor follow. No suppression yet — *both* cursors move, and that's expected.

This uses `rdev` (listen) + `enigo` (synthesize). It works on Mac and Windows with
zero FFI, so you validate the cross-platform path before investing in native hooks.

**Server (sketch):**

```rust
// bind TcpListener on 0.0.0.0:24800, accept one client, stream.set_nodelay(true)
// rdev::listen runs on the MAIN thread (required on macOS) and pushes mapped
// InputMsg values into an mpsc channel; a worker thread writes them to the socket.
listen(move |event| {
    let msg = match event.event_type {
        EventType::MouseMove { x, y } => InputMsg::MouseMove { x, y },
        EventType::ButtonPress(b)     => InputMsg::MouseButton { button: map(b), pressed: true },
        EventType::ButtonRelease(b)   => InputMsg::MouseButton { button: map(b), pressed: false },
        EventType::Wheel { delta_x, delta_y } => InputMsg::Wheel { dx: delta_x, dy: delta_y },
        _ => return,
    };
    let _ = tx.send(msg);
})
```

**Client (sketch):**

```rust
let mut enigo = Enigo::new(&Settings::default())?;
loop {
    match read_msg(&mut stream)? {
        InputMsg::MouseMove { x, y } => { enigo.move_mouse(x as i32, y as i32, Coordinate::Abs)?; }
        InputMsg::MouseButton { button, pressed } => {
            let dir = if pressed { Direction::Press } else { Direction::Release };
            enigo.button(map(button), dir)?;
        }
        InputMsg::Wheel { dx, dy } => { /* enigo.scroll(...) */ }
        _ => {}
    }
}
```

**Run it:**

```
# machine with the mouse:
cargo run --release -p kvm-server

# the other machine (pass the server's LAN IP):
cargo run --release -p kvm-client 192.168.1.50
```

Move the mouse on the server → the client cursor follows. You now have a working
cross-platform input pipe. Everything else is built on top of this.

> The crate versions (`rdev = "0.5"`, `enigo = "0.2"`) are pinned to match these
> signatures. `enigo`'s API has changed across releases; if the build complains
> about `move_mouse`/`button`/`scroll`, pin the exact version.

---

## 6. Phase 2 — Native capture, suppression & keyboard

This is the first hard phase. You replace `rdev` capture with **native hooks** so you
can *swallow* local events, and you add keyboard support with cross-OS mapping.

### 6.1 macOS capture — Quartz Event Services (`CGEventTap`)

Create a tap and run it on a `CFRunLoop`:

```
CGEvent.tapCreate(
    tap: .cgSessionEventTap,        // session-level
    place: .headInsertEventTap,     // see events before other handlers
    options: .defaultTap,           // CAN modify/suppress events
    eventsOfInterest: mouse + keyboard mask,
    callback: my_callback,
    userInfo: ...)
```

Key points:

- **`.defaultTap` vs `.listenOnly`** determines both capability *and* which permission
  is requested. `.defaultTap` lets you **return `nil` to swallow** an event (or pass it
  through) and requires the **Accessibility** permission. `.listenOnly` can only
  observe and requires **Input Monitoring**. A KVM server needs `.defaultTap`.
- **Suppress** while remote by returning `nil` from the callback. **Pass through** by
  returning the event unmodified.
- **Re-enable on disable:** the system can disable your tap
  (`kCGEventTapDisabledByTimeout` / `...ByUserInput`). Detect these event types and
  call `CGEvent.tapEnable(tap:enable:true)` again.
- **Fail safe:** if Accessibility permission is revoked *while the tap is active*, a
  naive implementation can swallow all input and lock the Mac. Always release the tap
  cleanly on permission loss. Check access with `CGPreflightListenEventAccess()` /
  `CGRequestListenEventAccess()` (or `IOHIDCheckAccess`/`IOHIDRequestAccess`).
- Calling Rust → these C APIs: use the `core-graphics` / `core-foundation` crates, or
  raw FFI via `objc2`/`core-foundation-sys`.

### 6.2 Windows capture — low-level hooks (`SetWindowsHookEx`)

```
SetWindowsHookEx(WH_KEYBOARD_LL, LowLevelKeyboardProc, hMod, 0);
SetWindowsHookEx(WH_MOUSE_LL,    LowLevelMouseProc,    hMod, 0);
```

Key points:

- The hook callback runs on the thread that installed it, so **that thread must run a
  message loop** (`GetMessage`/`PeekMessage` + `TranslateMessage`/`DispatchMessage`).
- **Suppress** by returning a non-zero value (do **not** call `CallNextHookEx`).
  **Pass through** by returning `CallNextHookEx(...)`.
- **Honour the timeout.** The callback must return within `LowLevelHooksTimeout`
  (default **1000 ms**; lowered from 30000 ms in Win10 1709). If you exceed it the
  hook is silently removed with no notification. **Do all serialize + network work
  off the hook thread** — the callback should only enqueue.
- Raw Input API is recommended by Microsoft for *monitoring*, but it cannot **block**
  events, so a suppressing KVM must use the LL hooks.

### 6.3 Synthesis recap (client)

- **macOS:** `CGEventCreateMouseEvent` / `CGEventCreateKeyboardEvent(src, keycode,
  keydown)`, set modifiers via `CGEventSetFlags`, post with
  `CGEventPost(.cghidEventTap, event)`. Requires **Accessibility**.
- **Windows:** fill `INPUT` structs and call `SendInput`. Prefer **scan codes**
  (`KEYEVENTF_SCANCODE`) over virtual keys for better app/game compatibility; set
  `KEYEVENTF_EXTENDEDKEY` for arrows/right-Ctrl/etc. For absolute motion use
  `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK` with coordinates normalized to
  0–65535.

(`enigo` already wraps most of this; you only drop to raw APIs when you need behaviour
it doesn't expose.)

### 6.4 Cross-OS keyboard mapping

This is where bugs live. Maintain a translation table between a **neutral key ID** and
each OS's keycodes. The thorniest part is **modifiers**:

- Map **macOS Cmd (⌘) ↔ Windows Ctrl** so ⌘C becomes Ctrl+C and vice versa.
- Map **macOS Option ↔ Windows Alt**, and handle the **Win/Super** key.
- Make the Cmd↔Ctrl swap **user-configurable** — preference varies.

Two strategies (offer both):

- **Scancode mode** — send the physical key position; the receiver's layout
  interprets it. Best for shortcut pass-through.
- **Unicode/text mode** — translate to characters before sending. Best for matching
  the *sender's* layout when typing text.

Edge cases to budget for: dead keys, AltGr, IME composition, non-US layouts, and lock
keys (Caps/Num/Scroll) which report state changes rather than clean press/release on
some systems.

### 6.5 Suppression state machine

```
state = LOCAL            // events flow to the server's own OS
on edge-cross to remote: state = REMOTE
while REMOTE:
    in capture callback: enqueue event for network, then SUPPRESS (nil / non-zero)
    keep warping the real cursor back to screen center (see Phase 3)
on edge-cross back to local: state = LOCAL, stop suppressing
```

---

## 7. Phase 3 — Edge transition & screen layout

**Goal:** make the cursor actually "leave" one machine and drive only the remote.

### 7.1 Layout graph

Model each screen as a node; each of its edges (or a percentage range of an edge) can
link to a neighbour:

```
MacBook  --right-->  WindowsPC
WindowsPC --left-->  MacBook      // links are DIRECTIONAL — define both ways
```

A config UI (drag screens into a grid) generates this graph. Treat a server with
multiple monitors as "one big screen."

### 7.2 Crossing logic

When the cursor hits a linked edge:

1. Switch `active_screen` to the neighbour.
2. **Warp the real cursor to the center** of the server screen
   (`CGWarpMouseCursorPosition` on macOS, `SetCursorPos` on Windows).
3. Compute all subsequent motion as **deltas from center** and send those deltas
   (switch `MouseMove` from absolute to relative). This stops the physical mouse from
   "running out of screen."
4. Hide the local cursor; send `ScreenEnter` to the target client.

> A real Synergy/Deskflow bug came from the macOS server failing to re-center on a
> hotkey switch, which made the remote cursor jump. Re-center on *every* switch path.

### 7.3 Anti-jitter guards

- **`switchDelay`** — cursor must rest on the edge N ms before switching.
- **`switchDoubleTap`** — require a quick double-bump of the edge.
- **`switchCorners`** — ignore the corners so diagonal motion doesn't switch
  accidentally.
- **Lock-to-screen hotkey** — a toggle (Synergy used Scroll-Lock) that pins the cursor
  to the current screen.

### 7.4 DPI / scaling

High-DPI and fractional display scaling corrupt naive position math (a classic Synergy
bug stuck the cursor in a corner on scaled Windows displays). Always compute in a
consistent coordinate space and account for each screen's scale factor.

---

## 8. Phase 4 — Clipboard, encryption, discovery, UX

### 8.1 Clipboard sync

Layer clipboard messages on the same connection. On a screen switch (or on a
clipboard-change notification), the owning machine grabs the clipboard and sends it so
the other side sets its local clipboard.

- **macOS:** `NSPasteboard`. There is no change notification, so **poll
  `changeCount`** (a monotonically increasing integer) at ~10 Hz and read when it
  changes.
- **Windows:** `WM_CLIPBOARDUPDATE` (add a clipboard format listener), read formats
  like `CF_UNICODETEXT`.
- File drag-and-drop is far more limited — start with text only; add single-file
  transfer later if needed.

### 8.2 Encryption & pairing (do this early, not last)

You are transmitting keystrokes. **Plaintext is not acceptable.** Synergy's old crypto
was publicly broken (key/IV reuse), and a plaintext server could be impersonated to
inject keystrokes into a client. Requirements:

- **Encrypt the channel:** TLS via **`rustls`**, or the **Noise protocol** via
  **`snow`** (used by WireGuard/Signal).
- **Authenticate/pair the machines** so a rogue peer can't join:
  - PIN-based: derive a symmetric key from a 6-digit PIN with **Argon2id**, then use
    **ChaCha20-Poly1305** for the session, or
  - TLS with **certificate-fingerprint pinning** (trust-on-first-use): keep an
    `authorized_fingerprints` list and reject unknown peers.
- **Restrict to the local subnet** and reject connections from off-LAN addresses.

### 8.3 Discovery

Offer both:

- **Manual IP/hostname** config (always works, no dependencies).
- **mDNS / DNS-SD** for zero-config: advertise a service type like
  `_yourkvm._tcp.local` with a TXT record (screen name, version). Use an in-process
  library (Rust **`mdns-sd`**) rather than depending on Apple's Bonjour-for-Windows
  service, which is unmaintained and flagged by Defender. Note multicast is often
  dropped on Wi-Fi and doesn't cross subnets — keep manual config as the fallback.

### 8.4 UX & lifecycle

- **System-tray app** with a layout editor (drag screens into a grid). `egui` or
  `Tauri` in Rust.
- **Auto-start:** `SMAppService` login item on macOS; a service or registry Run key on
  Windows (a service is also what lets you control the lock screen / elevated apps —
  see below).

---

## 9. OS permissions & hard limits

### macOS

- **TCC permissions:** the server needs **Accessibility** (to suppress/inject) and
  often **Input Monitoring** (to observe). They must be granted manually in
  System Settings → Privacy & Security and may require an app restart.
- **Re-signing invalidates trust:** TCC ties the grant to the app's code identity, so
  rebuilding/re-signing can silently revoke it ("the tap exists but receives no
  events"). Detect zero-event states and re-prompt.
- **Code signing / notarization:** unsigned apps are flagged "damaged"; without an
  Apple Developer ID + notarization, users must run `xattr -c YourApp.app`. Budget for
  a Developer ID for a real release.

### Windows

- **UAC / integrity levels:** low-level hooks and `SendInput` from a normal-privilege
  process **cannot see or inject into elevated windows** (UIPI blocks
  lower→higher-integrity input), and **cannot touch the UAC prompt or the
  logon/secure desktop at all**.
- **Mitigation:** install a **Windows service** running with appropriate privileges to
  control the lock screen / elevated apps (this is exactly why Mouse Without Borders
  offers "install as a service"). There is no way around the secure desktop except a
  privileged/system context — commit to the service early if this matters to you.
- **`Ctrl+Alt+Del`** can never be captured (it's handled by the secure kernel path);
  the established workaround is a separate hotkey that *sends* Ctrl+Alt+Del to a
  client.

---

## 10. Security checklist

- [ ] Channel encrypted (TLS via `rustls`, or Noise via `snow`).
- [ ] Peers authenticated (PIN-derived key, or cert-fingerprint pinning / TOFU).
- [ ] Connections restricted to the local subnet.
- [ ] Frame size capped; malformed frames rejected without crashing.
- [ ] No plaintext keystrokes on the wire, ever — including during early development.
- [ ] Heartbeat + timeout so a dropped peer can't leave modifiers/buttons stuck.
- [ ] Fail-safe on permission loss (release hooks; never lock the machine).

---

## 11. Testing & latency tuning

- **`TCP_NODELAY` is the single most important latency knob** — disable Nagle so small
  event packets aren't buffered. Set it on every socket.
- Do all heavy work **off** the capture callback thread (especially on Windows, where
  the hook has a 1000 ms budget).
- Target end-to-end input latency under ~30–50 ms on a LAN; above that it feels laggy
  and breaks the "one desktop" illusion.
- Test matrix: Mac→Mac, Mac→Win, Win→Win, Win→Mac. Test shortcuts (⌘C/Ctrl+C),
  modifier hold-through, scroll direction, multi-monitor servers, and HiDPI/scaled
  displays.
- Test permission-revocation paths explicitly (revoke macOS Accessibility while
  running; the app must not lock the Mac).

---

## 12. Packaging & distribution

- **macOS:** sign with a Developer ID certificate, **notarize**, and staple. Ship a
  `.app` (or `.dmg`). Without this, users hit Gatekeeper and must `xattr -c`.
- **Windows:** sign the binary (an EV or OV code-signing cert) to avoid SmartScreen
  warnings; ship an installer (MSI/Inno Setup) that can optionally register the
  service.
- Cross-compile with `cargo` targets; keep the server and client in one workspace so a
  single binary can run in either role.

---

## 13. Reference projects to study

Even building from scratch, read these for protocol design and OS edge cases:

| Project | Lang / License | Why study it |
|--------|----------------|--------------|
| **Deskflow** | C++20 / GPL-2.0 (+OpenSSL) | The live upstream of Synergy/Barrier/Input Leap; reference for edge transition, key mapping, clipboard, TLS, mDNS. |
| **Lan Mouse** | Rust / GPL-3.0 | Closest architecture to this guide; DTLS via WebRTC.rs; TOML `authorized_fingerprints`; study its capture backends. |
| **Input Leap** | C++ / GPL-2.0 (+OpenSSL) | Barrier's successor; good protocol reference (now largely stalled). |
| **PowerToys → Mouse Without Borders** | C# / MIT | Windows-only, but a clean reference for the **Windows service / secure-desktop** handling. |

> **License note:** Deskflow and Input Leap are GPL — you can read them for learning,
> but copying code into your own project pulls in GPL obligations. `enigo` (MIT) and
> Lan Mouse (GPL-3.0) have different terms. Keep your own implementation clean-room if
> you intend a non-GPL license.

---

## Suggested build order (summary)

1. **Phase 1** — `rdev` + `enigo`, mouse-move over TCP, hardcoded IP. Prove the pipe.
2. **Phase 2** — native hooks (`CGEventTap` `.defaultTap` / `SetWindowsHookEx`),
   suppression, keyboard, Cmd↔Ctrl mapping.
3. **Phase 3** — layout graph, edge crossing, warp-to-center + relative deltas, switch
   guards, lock hotkey, DPI handling.
4. **Phase 4** — clipboard, encryption + pairing, mDNS discovery, tray UI, auto-start,
   packaging/signing.

Most of your engineering effort will go into the **Phase 2 capture/suppression layer
per OS** and the **permission/signing hurdles** in Phase 4. The networking is the easy
part.
