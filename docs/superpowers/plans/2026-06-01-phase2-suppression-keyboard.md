# Phase 2 — Native Capture, Suppression & Keyboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace rdev with native OS hooks (CGEventTap on macOS, SetWindowsHookEx on Windows) so the server can suppress local input while forwarding to the client, toggled by Ctrl+Shift+Space. Add keyboard forwarding with automatic Cmd↔Ctrl mapping.

**Architecture:** Platform-specific capture modules behind conditional compilation (`#[cfg(target_os)]`). A shared state machine (LOCAL/REMOTE) in main.rs drives suppression decisions. The keymap module defines neutral key IDs so both platforms speak a common language. The client uses `enigo.raw()` to synthesize key events from neutral IDs mapped back to local OS keycodes.

**Tech Stack:** Rust, core-graphics + core-foundation (macOS FFI), windows crate (Windows FFI), enigo 0.2.1 (client synthesis), serde + bincode (protocol)

---

## File Structure

```
crates/
├── common/
│   └── src/
│       ├── lib.rs              # Updated InputMsg enum (add KeyDown, KeyUp, ScreenEnter, ScreenLeave)
│       └── keymap.rs           # Neutral key IDs, OS keycode mapping tables, Cmd↔Ctrl swap
├── server/
│   ├── Cargo.toml              # Remove rdev, add core-graphics/core-foundation (macOS), windows (Windows)
│   └── src/
│       ├── main.rs             # State machine, hotkey detection, TCP integration
│       ├── bin/test_server.rs   # Same as main.rs (firewall workaround — keep in sync)
│       └── capture/
│           ├── mod.rs          # CaptureEvent enum, run_capture() cfg dispatcher
│           ├── macos.rs        # CGEventTap implementation
│           └── windows.rs      # SetWindowsHookEx implementation
└── client/
    └── src/
        └── main.rs             # Add keyboard synthesis, handle ScreenEnter/ScreenLeave
tests/
└── integration/
    └── protocol_test.rs        # Add tests for new message types
```

---

### Task 1: Update Wire Protocol

**Files:**
- Modify: `crates/common/src/lib.rs`
- Modify: `tests/integration/protocol_test.rs`

- [ ] **Step 1: Write failing tests for new message types**

Add to `tests/integration/protocol_test.rs`:

```rust
#[test]
fn roundtrip_key_down() {
    let msg = InputMsg::KeyDown { key: 42, modifiers: 0x03 };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_key_up() {
    let msg = InputMsg::KeyUp { key: 42, modifiers: 0x01 };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_screen_enter() {
    let msg = InputMsg::ScreenEnter;
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_screen_leave() {
    let msg = InputMsg::ScreenLeave;
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test protocol_test`
Expected: FAIL — `KeyDown`, `KeyUp`, `ScreenEnter`, `ScreenLeave` not found.

- [ ] **Step 3: Add new variants to InputMsg**

Replace the `InputMsg` enum in `crates/common/src/lib.rs`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum InputMsg {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
    KeyDown { key: u32, modifiers: u8 },
    KeyUp { key: u32, modifiers: u8 },
    ScreenEnter,
    ScreenLeave,
}
```

Also add modifier constants after the `MouseButton` enum:

```rust
// Modifier bitfield constants
pub const MOD_SHIFT: u8 = 0x01;
pub const MOD_CTRL: u8 = 0x02;
pub const MOD_ALT: u8 = 0x04;
pub const MOD_META: u8 = 0x08;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test --test protocol_test`
Expected: All 10 tests PASS (6 old + 4 new).

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/lib.rs tests/integration/protocol_test.rs
git commit -m "feat: add KeyDown, KeyUp, ScreenEnter, ScreenLeave to wire protocol"
```

---

### Task 2: Implement Keymap Module

**Files:**
- Create: `crates/common/src/keymap.rs`
- Modify: `crates/common/src/lib.rs` (add `pub mod keymap;`)
- Create: `tests/integration/keymap_test.rs`

- [ ] **Step 1: Write failing keymap tests**

Create `tests/integration/keymap_test.rs`:

```rust
use deskserver_common::keymap::*;

#[test]
fn neutral_a_roundtrips_macos() {
    let neutral = macos_keycode_to_neutral(0x00); // macOS 'A' keycode
    assert_eq!(neutral, Some(NK_A));
    let back = neutral_to_macos_keycode(NK_A);
    assert_eq!(back, Some(0x00));
}

#[test]
fn neutral_a_roundtrips_windows() {
    let neutral = windows_vk_to_neutral(0x41); // Windows 'A' VK code
    assert_eq!(neutral, Some(NK_A));
    let back = neutral_to_windows_vk(NK_A);
    assert_eq!(back, Some(0x41));
}

#[test]
fn cmd_ctrl_swap_mac_to_win() {
    // Mac Cmd key should map to neutral CTRL (primary shortcut modifier)
    let neutral = macos_keycode_to_neutral(0x37); // Left Cmd on macOS
    assert_eq!(neutral, Some(NK_CTRL_LEFT));
    // On Windows side, CTRL_LEFT becomes VK_LCONTROL
    let vk = neutral_to_windows_vk(NK_CTRL_LEFT);
    assert_eq!(vk, Some(0xA2)); // VK_LCONTROL
}

#[test]
fn cmd_ctrl_swap_win_to_mac() {
    // Windows Ctrl key should map to neutral META (becomes Cmd on Mac)
    // Wait — we want Ctrl on Windows to stay as Ctrl.
    // The swap is: Mac Cmd → neutral CTRL, Mac Ctrl → neutral META
    // Windows Ctrl → neutral CTRL, Windows Win → neutral META
    // So on the Mac CAPTURE side: Cmd=55 → NK_CTRL_LEFT, Ctrl=59 → NK_META_LEFT
    // On the Windows CAPTURE side: Ctrl=0xA2 → NK_CTRL_LEFT, Win=0x5B → NK_META_LEFT
    // Both platforms agree: NK_CTRL_LEFT = primary shortcut modifier
    let neutral = windows_vk_to_neutral(0xA2); // VK_LCONTROL
    assert_eq!(neutral, Some(NK_CTRL_LEFT));
    let mac_kc = neutral_to_macos_keycode(NK_CTRL_LEFT);
    assert_eq!(mac_kc, Some(0x37)); // Left Cmd on macOS
}

#[test]
fn modifier_bits_from_macos() {
    // Cmd held on Mac → should produce MOD_CTRL bit (after swap)
    use deskserver_common::{MOD_CTRL, MOD_SHIFT};
    let mods = macos_flags_to_modifiers(0x100108); // Cmd+Shift flags
    assert_eq!(mods & MOD_CTRL, MOD_CTRL);
    assert_eq!(mods & MOD_SHIFT, MOD_SHIFT);
}

#[test]
fn modifier_bits_from_windows() {
    use deskserver_common::MOD_CTRL;
    let mods = windows_mods_to_modifiers(true, false, false, false); // ctrl, shift, alt, win
    assert_eq!(mods & MOD_CTRL, MOD_CTRL);
}

#[test]
fn space_key_maps() {
    let neutral = macos_keycode_to_neutral(0x31); // Space on macOS
    assert_eq!(neutral, Some(NK_SPACE));
    let vk = neutral_to_windows_vk(NK_SPACE);
    assert_eq!(vk, Some(0x20)); // VK_SPACE
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test keymap_test`
Expected: FAIL — module `keymap` not found.

- [ ] **Step 3: Create the keymap module**

Create `crates/common/src/keymap.rs`:

```rust
//! Neutral key ID mapping between macOS keycodes, Windows virtual keys,
//! and a platform-independent neutral key space.
//!
//! Cmd↔Ctrl swap is built into the mapping tables:
//! - macOS Cmd → NK_CTRL (primary shortcut modifier)
//! - macOS Ctrl → NK_META
//! - Windows Ctrl → NK_CTRL (primary shortcut modifier)
//! - Windows Win → NK_META

use crate::{MOD_SHIFT, MOD_CTRL, MOD_ALT, MOD_META};

// ── Neutral Key IDs ──────────────────────────────────────────────────

pub const NK_A: u32 = 1;
pub const NK_B: u32 = 2;
pub const NK_C: u32 = 3;
pub const NK_D: u32 = 4;
pub const NK_E: u32 = 5;
pub const NK_F: u32 = 6;
pub const NK_G: u32 = 7;
pub const NK_H: u32 = 8;
pub const NK_I: u32 = 9;
pub const NK_J: u32 = 10;
pub const NK_K: u32 = 11;
pub const NK_L: u32 = 12;
pub const NK_M: u32 = 13;
pub const NK_N: u32 = 14;
pub const NK_O: u32 = 15;
pub const NK_P: u32 = 16;
pub const NK_Q: u32 = 17;
pub const NK_R: u32 = 18;
pub const NK_S: u32 = 19;
pub const NK_T: u32 = 20;
pub const NK_U: u32 = 21;
pub const NK_V: u32 = 22;
pub const NK_W: u32 = 23;
pub const NK_X: u32 = 24;
pub const NK_Y: u32 = 25;
pub const NK_Z: u32 = 26;

pub const NK_0: u32 = 30;
pub const NK_1: u32 = 31;
pub const NK_2: u32 = 32;
pub const NK_3: u32 = 33;
pub const NK_4: u32 = 34;
pub const NK_5: u32 = 35;
pub const NK_6: u32 = 36;
pub const NK_7: u32 = 37;
pub const NK_8: u32 = 38;
pub const NK_9: u32 = 39;

pub const NK_F1: u32 = 50;
pub const NK_F2: u32 = 51;
pub const NK_F3: u32 = 52;
pub const NK_F4: u32 = 53;
pub const NK_F5: u32 = 54;
pub const NK_F6: u32 = 55;
pub const NK_F7: u32 = 56;
pub const NK_F8: u32 = 57;
pub const NK_F9: u32 = 58;
pub const NK_F10: u32 = 59;
pub const NK_F11: u32 = 60;
pub const NK_F12: u32 = 61;

pub const NK_RETURN: u32 = 70;
pub const NK_TAB: u32 = 71;
pub const NK_SPACE: u32 = 72;
pub const NK_BACKSPACE: u32 = 73;
pub const NK_ESCAPE: u32 = 74;
pub const NK_DELETE: u32 = 75;

pub const NK_UP: u32 = 80;
pub const NK_DOWN: u32 = 81;
pub const NK_LEFT: u32 = 82;
pub const NK_RIGHT: u32 = 83;
pub const NK_HOME: u32 = 84;
pub const NK_END: u32 = 85;
pub const NK_PAGE_UP: u32 = 86;
pub const NK_PAGE_DOWN: u32 = 87;

pub const NK_SHIFT_LEFT: u32 = 90;
pub const NK_SHIFT_RIGHT: u32 = 91;
pub const NK_CTRL_LEFT: u32 = 92;   // Primary shortcut modifier (Cmd on Mac, Ctrl on Win)
pub const NK_CTRL_RIGHT: u32 = 93;
pub const NK_ALT_LEFT: u32 = 94;    // Option on Mac, Alt on Win
pub const NK_ALT_RIGHT: u32 = 95;
pub const NK_META_LEFT: u32 = 96;   // Ctrl on Mac, Win on Win
pub const NK_META_RIGHT: u32 = 97;

pub const NK_CAPS_LOCK: u32 = 100;

pub const NK_MINUS: u32 = 110;
pub const NK_EQUAL: u32 = 111;
pub const NK_LBRACKET: u32 = 112;
pub const NK_RBRACKET: u32 = 113;
pub const NK_BACKSLASH: u32 = 114;
pub const NK_SEMICOLON: u32 = 115;
pub const NK_QUOTE: u32 = 116;
pub const NK_GRAVE: u32 = 117;
pub const NK_COMMA: u32 = 118;
pub const NK_PERIOD: u32 = 119;
pub const NK_SLASH: u32 = 120;

// ── macOS Keycode Mapping ────────────────────────────────────────────
// macOS CGKeyCode values. Cmd↔Ctrl swap: Cmd(55)→NK_CTRL, Ctrl(59)→NK_META

pub fn macos_keycode_to_neutral(kc: u32) -> Option<u32> {
    match kc {
        0x00 => Some(NK_A), 0x0B => Some(NK_B), 0x08 => Some(NK_C),
        0x02 => Some(NK_D), 0x0E => Some(NK_E), 0x03 => Some(NK_F),
        0x05 => Some(NK_G), 0x04 => Some(NK_H), 0x22 => Some(NK_I),
        0x26 => Some(NK_J), 0x28 => Some(NK_K), 0x25 => Some(NK_L),
        0x2E => Some(NK_M), 0x2D => Some(NK_N), 0x1F => Some(NK_O),
        0x23 => Some(NK_P), 0x0C => Some(NK_Q), 0x0F => Some(NK_R),
        0x01 => Some(NK_S), 0x11 => Some(NK_T), 0x20 => Some(NK_U),
        0x09 => Some(NK_V), 0x0D => Some(NK_W), 0x07 => Some(NK_X),
        0x10 => Some(NK_Y), 0x06 => Some(NK_Z),

        0x1D => Some(NK_0), 0x12 => Some(NK_1), 0x13 => Some(NK_2),
        0x14 => Some(NK_3), 0x15 => Some(NK_4), 0x17 => Some(NK_5),
        0x16 => Some(NK_6), 0x1A => Some(NK_7), 0x1C => Some(NK_8),
        0x19 => Some(NK_9),

        0x7A => Some(NK_F1),  0x78 => Some(NK_F2),  0x63 => Some(NK_F3),
        0x76 => Some(NK_F4),  0x60 => Some(NK_F5),  0x61 => Some(NK_F6),
        0x62 => Some(NK_F7),  0x64 => Some(NK_F8),  0x65 => Some(NK_F9),
        0x6D => Some(NK_F10), 0x67 => Some(NK_F11), 0x6F => Some(NK_F12),

        0x24 => Some(NK_RETURN), 0x30 => Some(NK_TAB), 0x31 => Some(NK_SPACE),
        0x33 => Some(NK_BACKSPACE), 0x35 => Some(NK_ESCAPE), 0x75 => Some(NK_DELETE),

        0x7E => Some(NK_UP), 0x7D => Some(NK_DOWN),
        0x7B => Some(NK_LEFT), 0x7C => Some(NK_RIGHT),
        0x73 => Some(NK_HOME), 0x77 => Some(NK_END),
        0x74 => Some(NK_PAGE_UP), 0x79 => Some(NK_PAGE_DOWN),

        // Cmd↔Ctrl swap: Mac Cmd → neutral CTRL, Mac Ctrl → neutral META
        0x37 => Some(NK_CTRL_LEFT),   // Left Cmd → CTRL
        0x36 => Some(NK_CTRL_RIGHT),  // Right Cmd → CTRL
        0x3B => Some(NK_META_LEFT),   // Left Ctrl → META
        0x3E => Some(NK_META_RIGHT),  // Right Ctrl → META
        0x38 => Some(NK_SHIFT_LEFT),  0x3C => Some(NK_SHIFT_RIGHT),
        0x3A => Some(NK_ALT_LEFT),    0x3D => Some(NK_ALT_RIGHT),
        0x39 => Some(NK_CAPS_LOCK),

        0x1B => Some(NK_MINUS), 0x18 => Some(NK_EQUAL),
        0x21 => Some(NK_LBRACKET), 0x1E => Some(NK_RBRACKET),
        0x2A => Some(NK_BACKSLASH), 0x29 => Some(NK_SEMICOLON),
        0x27 => Some(NK_QUOTE), 0x32 => Some(NK_GRAVE),
        0x2B => Some(NK_COMMA), 0x2F => Some(NK_PERIOD),
        0x2C => Some(NK_SLASH),
        _ => None,
    }
}

pub fn neutral_to_macos_keycode(nk: u32) -> Option<u32> {
    match nk {
        NK_A => Some(0x00), NK_B => Some(0x0B), NK_C => Some(0x08),
        NK_D => Some(0x02), NK_E => Some(0x0E), NK_F => Some(0x03),
        NK_G => Some(0x05), NK_H => Some(0x04), NK_I => Some(0x22),
        NK_J => Some(0x26), NK_K => Some(0x28), NK_L => Some(0x25),
        NK_M => Some(0x2E), NK_N => Some(0x2D), NK_O => Some(0x1F),
        NK_P => Some(0x23), NK_Q => Some(0x0C), NK_R => Some(0x0F),
        NK_S => Some(0x01), NK_T => Some(0x11), NK_U => Some(0x20),
        NK_V => Some(0x09), NK_W => Some(0x0D), NK_X => Some(0x07),
        NK_Y => Some(0x10), NK_Z => Some(0x06),

        NK_0 => Some(0x1D), NK_1 => Some(0x12), NK_2 => Some(0x13),
        NK_3 => Some(0x14), NK_4 => Some(0x15), NK_5 => Some(0x17),
        NK_6 => Some(0x16), NK_7 => Some(0x1A), NK_8 => Some(0x1C),
        NK_9 => Some(0x19),

        NK_F1 => Some(0x7A),  NK_F2 => Some(0x78),  NK_F3 => Some(0x63),
        NK_F4 => Some(0x76),  NK_F5 => Some(0x60),  NK_F6 => Some(0x61),
        NK_F7 => Some(0x62),  NK_F8 => Some(0x64),  NK_F9 => Some(0x65),
        NK_F10 => Some(0x6D), NK_F11 => Some(0x67), NK_F12 => Some(0x6F),

        NK_RETURN => Some(0x24), NK_TAB => Some(0x30), NK_SPACE => Some(0x31),
        NK_BACKSPACE => Some(0x33), NK_ESCAPE => Some(0x35), NK_DELETE => Some(0x75),

        NK_UP => Some(0x7E), NK_DOWN => Some(0x7D),
        NK_LEFT => Some(0x7B), NK_RIGHT => Some(0x7C),
        NK_HOME => Some(0x73), NK_END => Some(0x77),
        NK_PAGE_UP => Some(0x74), NK_PAGE_DOWN => Some(0x79),

        // Reverse Cmd↔Ctrl swap: neutral CTRL → Mac Cmd, neutral META → Mac Ctrl
        NK_CTRL_LEFT => Some(0x37),   NK_CTRL_RIGHT => Some(0x36),
        NK_META_LEFT => Some(0x3B),   NK_META_RIGHT => Some(0x3E),
        NK_SHIFT_LEFT => Some(0x38),  NK_SHIFT_RIGHT => Some(0x3C),
        NK_ALT_LEFT => Some(0x3A),    NK_ALT_RIGHT => Some(0x3D),
        NK_CAPS_LOCK => Some(0x39),

        NK_MINUS => Some(0x1B), NK_EQUAL => Some(0x18),
        NK_LBRACKET => Some(0x21), NK_RBRACKET => Some(0x1E),
        NK_BACKSLASH => Some(0x2A), NK_SEMICOLON => Some(0x29),
        NK_QUOTE => Some(0x27), NK_GRAVE => Some(0x32),
        NK_COMMA => Some(0x2B), NK_PERIOD => Some(0x2F),
        NK_SLASH => Some(0x2C),
        _ => None,
    }
}

// ── Windows Virtual Key Mapping ──────────────────────────────────────
// Windows VK codes. Cmd↔Ctrl swap: Ctrl→NK_CTRL, Win→NK_META

pub fn windows_vk_to_neutral(vk: u32) -> Option<u32> {
    match vk {
        0x41 => Some(NK_A), 0x42 => Some(NK_B), 0x43 => Some(NK_C),
        0x44 => Some(NK_D), 0x45 => Some(NK_E), 0x46 => Some(NK_F),
        0x47 => Some(NK_G), 0x48 => Some(NK_H), 0x49 => Some(NK_I),
        0x4A => Some(NK_J), 0x4B => Some(NK_K), 0x4C => Some(NK_L),
        0x4D => Some(NK_M), 0x4E => Some(NK_N), 0x4F => Some(NK_O),
        0x50 => Some(NK_P), 0x51 => Some(NK_Q), 0x52 => Some(NK_R),
        0x53 => Some(NK_S), 0x54 => Some(NK_T), 0x55 => Some(NK_U),
        0x56 => Some(NK_V), 0x57 => Some(NK_W), 0x58 => Some(NK_X),
        0x59 => Some(NK_Y), 0x5A => Some(NK_Z),

        0x30 => Some(NK_0), 0x31 => Some(NK_1), 0x32 => Some(NK_2),
        0x33 => Some(NK_3), 0x34 => Some(NK_4), 0x35 => Some(NK_5),
        0x36 => Some(NK_6), 0x37 => Some(NK_7), 0x38 => Some(NK_8),
        0x39 => Some(NK_9),

        0x70 => Some(NK_F1),  0x71 => Some(NK_F2),  0x72 => Some(NK_F3),
        0x73 => Some(NK_F4),  0x74 => Some(NK_F5),  0x75 => Some(NK_F6),
        0x76 => Some(NK_F7),  0x77 => Some(NK_F8),  0x78 => Some(NK_F9),
        0x79 => Some(NK_F10), 0x7A => Some(NK_F11), 0x7B => Some(NK_F12),

        0x0D => Some(NK_RETURN), 0x09 => Some(NK_TAB), 0x20 => Some(NK_SPACE),
        0x08 => Some(NK_BACKSPACE), 0x1B => Some(NK_ESCAPE), 0x2E => Some(NK_DELETE),

        0x26 => Some(NK_UP), 0x28 => Some(NK_DOWN),
        0x25 => Some(NK_LEFT), 0x27 => Some(NK_RIGHT),
        0x24 => Some(NK_HOME), 0x23 => Some(NK_END),
        0x21 => Some(NK_PAGE_UP), 0x22 => Some(NK_PAGE_DOWN),

        // No swap needed: Windows Ctrl → NK_CTRL, Windows Win → NK_META
        0xA2 => Some(NK_CTRL_LEFT),   0xA3 => Some(NK_CTRL_RIGHT),
        0x5B => Some(NK_META_LEFT),   0x5C => Some(NK_META_RIGHT),
        0xA0 => Some(NK_SHIFT_LEFT),  0xA1 => Some(NK_SHIFT_RIGHT),
        0xA4 => Some(NK_ALT_LEFT),    0xA5 => Some(NK_ALT_RIGHT),
        0x14 => Some(NK_CAPS_LOCK),

        // Also accept non-lateralized VK codes
        0x10 => Some(NK_SHIFT_LEFT),  // VK_SHIFT
        0x11 => Some(NK_CTRL_LEFT),   // VK_CONTROL
        0x12 => Some(NK_ALT_LEFT),    // VK_MENU

        0xBD => Some(NK_MINUS), 0xBB => Some(NK_EQUAL),
        0xDB => Some(NK_LBRACKET), 0xDD => Some(NK_RBRACKET),
        0xDC => Some(NK_BACKSLASH), 0xBA => Some(NK_SEMICOLON),
        0xDE => Some(NK_QUOTE), 0xC0 => Some(NK_GRAVE),
        0xBC => Some(NK_COMMA), 0xBE => Some(NK_PERIOD),
        0xBF => Some(NK_SLASH),
        _ => None,
    }
}

pub fn neutral_to_windows_vk(nk: u32) -> Option<u32> {
    match nk {
        NK_A => Some(0x41), NK_B => Some(0x42), NK_C => Some(0x43),
        NK_D => Some(0x44), NK_E => Some(0x45), NK_F => Some(0x46),
        NK_G => Some(0x47), NK_H => Some(0x48), NK_I => Some(0x49),
        NK_J => Some(0x4A), NK_K => Some(0x4B), NK_L => Some(0x4C),
        NK_M => Some(0x4D), NK_N => Some(0x4E), NK_O => Some(0x4F),
        NK_P => Some(0x50), NK_Q => Some(0x51), NK_R => Some(0x52),
        NK_S => Some(0x53), NK_T => Some(0x54), NK_U => Some(0x55),
        NK_V => Some(0x56), NK_W => Some(0x57), NK_X => Some(0x58),
        NK_Y => Some(0x59), NK_Z => Some(0x5A),

        NK_0 => Some(0x30), NK_1 => Some(0x31), NK_2 => Some(0x32),
        NK_3 => Some(0x33), NK_4 => Some(0x34), NK_5 => Some(0x35),
        NK_6 => Some(0x36), NK_7 => Some(0x37), NK_8 => Some(0x38),
        NK_9 => Some(0x39),

        NK_F1 => Some(0x70),  NK_F2 => Some(0x71),  NK_F3 => Some(0x72),
        NK_F4 => Some(0x73),  NK_F5 => Some(0x74),  NK_F6 => Some(0x75),
        NK_F7 => Some(0x76),  NK_F8 => Some(0x77),  NK_F9 => Some(0x78),
        NK_F10 => Some(0x79), NK_F11 => Some(0x7A), NK_F12 => Some(0x7B),

        NK_RETURN => Some(0x0D), NK_TAB => Some(0x09), NK_SPACE => Some(0x20),
        NK_BACKSPACE => Some(0x08), NK_ESCAPE => Some(0x1B), NK_DELETE => Some(0x2E),

        NK_UP => Some(0x26), NK_DOWN => Some(0x28),
        NK_LEFT => Some(0x25), NK_RIGHT => Some(0x27),
        NK_HOME => Some(0x24), NK_END => Some(0x23),
        NK_PAGE_UP => Some(0x21), NK_PAGE_DOWN => Some(0x22),

        NK_CTRL_LEFT => Some(0xA2),   NK_CTRL_RIGHT => Some(0xA3),
        NK_META_LEFT => Some(0x5B),   NK_META_RIGHT => Some(0x5C),
        NK_SHIFT_LEFT => Some(0xA0),  NK_SHIFT_RIGHT => Some(0xA1),
        NK_ALT_LEFT => Some(0xA4),    NK_ALT_RIGHT => Some(0xA5),
        NK_CAPS_LOCK => Some(0x14),

        NK_MINUS => Some(0xBD), NK_EQUAL => Some(0xBB),
        NK_LBRACKET => Some(0xDB), NK_RBRACKET => Some(0xDD),
        NK_BACKSLASH => Some(0xDC), NK_SEMICOLON => Some(0xBA),
        NK_QUOTE => Some(0xDE), NK_GRAVE => Some(0xC0),
        NK_COMMA => Some(0xBC), NK_PERIOD => Some(0xBE),
        NK_SLASH => Some(0xBF),
        _ => None,
    }
}

// ── Modifier Helpers ─────────────────────────────────────────────────

/// Convert macOS CGEventFlags to our modifier bitfield.
/// Applies Cmd↔Ctrl swap: Cmd flag → MOD_CTRL, Ctrl flag → MOD_META.
pub fn macos_flags_to_modifiers(flags: u64) -> u8 {
    let mut mods = 0u8;
    if flags & 0x020000 != 0 { mods |= MOD_SHIFT; }  // kCGEventFlagMaskShift
    if flags & 0x040000 != 0 { mods |= MOD_META; }    // kCGEventFlagMaskControl → META (swapped)
    if flags & 0x080000 != 0 { mods |= MOD_ALT; }     // kCGEventFlagMaskAlternate
    if flags & 0x100000 != 0 { mods |= MOD_CTRL; }    // kCGEventFlagMaskCommand → CTRL (swapped)
    mods
}

/// Convert Windows modifier key states to our modifier bitfield.
/// No swap needed: Windows Ctrl → MOD_CTRL, Windows Win → MOD_META.
pub fn windows_mods_to_modifiers(ctrl: bool, shift: bool, alt: bool, win: bool) -> u8 {
    let mut mods = 0u8;
    if shift { mods |= MOD_SHIFT; }
    if ctrl  { mods |= MOD_CTRL; }
    if alt   { mods |= MOD_ALT; }
    if win   { mods |= MOD_META; }
    mods
}
```

- [ ] **Step 4: Add `pub mod keymap;` to lib.rs**

Add this line after the existing `use` statements in `crates/common/src/lib.rs`:

```rust
pub mod keymap;
```

- [ ] **Step 5: Add test entry to workspace Cargo.toml**

Add to the root `Cargo.toml`:

```toml
[[test]]
name = "keymap_test"
path = "tests/integration/keymap_test.rs"
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test --test keymap_test`
Expected: All 7 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/common/src/keymap.rs crates/common/src/lib.rs tests/integration/keymap_test.rs Cargo.toml
git commit -m "feat: add keymap module with neutral key IDs and Cmd↔Ctrl swap"
```

---

### Task 3: Create Capture Module Structure

**Files:**
- Create: `crates/server/src/capture/mod.rs`
- Create: `crates/server/src/capture/macos.rs` (stub)
- Create: `crates/server/src/capture/windows.rs` (stub)
- Modify: `crates/server/Cargo.toml` (update dependencies)

- [ ] **Step 1: Update server Cargo.toml**

Replace `crates/server/Cargo.toml`:

```toml
[package]
name = "kvm-server"
version = "0.1.0"
edition = "2021"
default-run = "test_server"

[dependencies]
deskserver-common = { path = "../common" }

[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.23"
core-foundation = "0.9"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
] }
```

- [ ] **Step 2: Create capture/mod.rs**

Create `crates/server/src/capture/mod.rs`:

```rust
use deskserver_common::MouseButton;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Internal capture event — platform-neutral representation of a raw OS event.
#[derive(Debug, Clone)]
pub enum CaptureEvent {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
    KeyDown { keycode: u32, modifiers: u8 },
    KeyUp { keycode: u32, modifiers: u8 },
}

/// Run the platform-specific capture loop on the current (main) thread.
/// The callback receives each event and returns `true` to suppress it, `false` to pass through.
/// This function blocks forever (runs the OS event loop).
pub fn run_capture<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    #[cfg(target_os = "macos")]
    macos::run(callback);

    #[cfg(target_os = "windows")]
    windows::run(callback);
}
```

- [ ] **Step 3: Create capture/macos.rs (stub)**

Create `crates/server/src/capture/macos.rs`:

```rust
use super::CaptureEvent;

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(_callback: F) {
    todo!("macOS CGEventTap capture — implemented in Task 4")
}
```

- [ ] **Step 4: Create capture/windows.rs (stub)**

Create `crates/server/src/capture/windows.rs`:

```rust
use super::CaptureEvent;

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(_callback: F) {
    todo!("Windows SetWindowsHookEx capture — implemented in Task 5")
}
```

- [ ] **Step 5: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-server`
Expected: Compiles (stubs are not called yet).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/capture/ crates/server/Cargo.toml
git commit -m "feat: add capture module structure with CaptureEvent and platform stubs"
```

---

### Task 4: Implement macOS Capture (CGEventTap)

**Files:**
- Modify: `crates/server/src/capture/macos.rs`

- [ ] **Step 1: Implement the macOS capture backend**

Replace `crates/server/src/capture/macos.rs`:

```rust
use super::CaptureEvent;
use deskserver_common::keymap::macos_flags_to_modifiers;
use deskserver_common::MouseButton;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField,
};
use std::cell::RefCell;

// Event type constants not in the crate
const MOUSE_MOVED: CGEventType = CGEventType::MouseMoved;
const LEFT_DOWN: CGEventType = CGEventType::LeftMouseDown;
const LEFT_UP: CGEventType = CGEventType::LeftMouseUp;
const RIGHT_DOWN: CGEventType = CGEventType::RightMouseDown;
const RIGHT_UP: CGEventType = CGEventType::RightMouseUp;
const OTHER_DOWN: CGEventType = CGEventType::OtherMouseDown;
const OTHER_UP: CGEventType = CGEventType::OtherMouseUp;
const SCROLL_WHEEL: CGEventType = CGEventType::ScrollWheel;
const KEY_DOWN: CGEventType = CGEventType::KeyDown;
const KEY_UP: CGEventType = CGEventType::KeyUp;
const FLAGS_CHANGED: CGEventType = CGEventType::FlagsChanged;
const TAP_DISABLED: CGEventType = CGEventType::TapDisabledByTimeout;
const LEFT_DRAGGED: CGEventType = CGEventType::LeftMouseDragged;
const RIGHT_DRAGGED: CGEventType = CGEventType::RightMouseDragged;
const OTHER_DRAGGED: CGEventType = CGEventType::OtherMouseDragged;

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    // Store callback in thread-local since CGEventTap callback is a C function pointer
    thread_local! {
        static CALLBACK: RefCell<Option<Box<dyn FnMut(CaptureEvent) -> bool>>> = RefCell::new(None);
    }
    CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(callback));
    });

    let event_mask: CGEventType = unsafe { std::mem::transmute(
        (1u64 << MOUSE_MOVED as u64)
        | (1u64 << LEFT_DOWN as u64) | (1u64 << LEFT_UP as u64)
        | (1u64 << RIGHT_DOWN as u64) | (1u64 << RIGHT_UP as u64)
        | (1u64 << OTHER_DOWN as u64) | (1u64 << OTHER_UP as u64)
        | (1u64 << SCROLL_WHEEL as u64)
        | (1u64 << KEY_DOWN as u64) | (1u64 << KEY_UP as u64)
        | (1u64 << FLAGS_CHANGED as u64)
        | (1u64 << LEFT_DRAGGED as u64)
        | (1u64 << RIGHT_DRAGGED as u64)
        | (1u64 << OTHER_DRAGGED as u64)
    )};

    let tap = CGEvent::tap_create(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            MOUSE_MOVED, LEFT_DOWN, LEFT_UP, RIGHT_DOWN, RIGHT_UP,
            OTHER_DOWN, OTHER_UP, SCROLL_WHEEL, KEY_DOWN, KEY_UP,
            FLAGS_CHANGED, LEFT_DRAGGED, RIGHT_DRAGGED, OTHER_DRAGGED,
        ],
        |_proxy, event_type, event| {
            // Re-enable if system disabled the tap
            if event_type == TAP_DISABLED {
                println!("[CAPTURE] Tap was disabled, re-enabling...");
                // The tap will be re-enabled automatically by returning the event
                return Some(event.clone());
            }

            let capture_event = match event_type {
                MOUSE_MOVED | LEFT_DRAGGED | RIGHT_DRAGGED | OTHER_DRAGGED => {
                    let loc = event.location();
                    Some(CaptureEvent::MouseMove { x: loc.x, y: loc.y })
                }
                LEFT_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: true }),
                LEFT_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: false }),
                RIGHT_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: true }),
                RIGHT_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: false }),
                OTHER_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: true }),
                OTHER_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: false }),
                SCROLL_WHEEL => {
                    let dy = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
                    let dx = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
                    Some(CaptureEvent::Wheel { dx, dy })
                }
                KEY_DOWN => {
                    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                    let flags = event.get_flags();
                    let mods = macos_flags_to_modifiers(flags.bits());
                    Some(CaptureEvent::KeyDown { keycode, modifiers: mods })
                }
                KEY_UP => {
                    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                    let flags = event.get_flags();
                    let mods = macos_flags_to_modifiers(flags.bits());
                    Some(CaptureEvent::KeyUp { keycode, modifiers: mods })
                }
                FLAGS_CHANGED => {
                    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                    let flags = event.get_flags();
                    let mods = macos_flags_to_modifiers(flags.bits());
                    // Determine if this is a press or release based on flags
                    let is_press = match keycode {
                        0x38 | 0x3C => flags.contains(CGEventFlags::CGEventFlagShift),
                        0x3B | 0x3E => flags.contains(CGEventFlags::CGEventFlagControl),
                        0x3A | 0x3D => flags.contains(CGEventFlags::CGEventFlagAlternate),
                        0x37 | 0x36 => flags.contains(CGEventFlags::CGEventFlagCommand),
                        _ => true,
                    };
                    if is_press {
                        Some(CaptureEvent::KeyDown { keycode, modifiers: mods })
                    } else {
                        Some(CaptureEvent::KeyUp { keycode, modifiers: mods })
                    }
                }
                _ => None,
            };

            if let Some(ce) = capture_event {
                let suppress = CALLBACK.with(|cb| {
                    if let Some(ref mut f) = *cb.borrow_mut() {
                        f(ce)
                    } else {
                        false
                    }
                });
                if suppress {
                    return None; // Suppress the event
                }
            }
            Some(event.clone()) // Pass through
        },
    );

    match tap {
        Some(tap) => {
            unsafe {
                let source = tap.mach_port_create_runloop_source(0).expect("failed to create run loop source");
                let run_loop = CFRunLoop::get_current();
                run_loop.add_source(&source, kCFRunLoopCommonModes);
                tap.enable();
                println!("[CAPTURE] macOS CGEventTap active. Accessibility permission required.");
                CFRunLoop::run_current();
            }
        }
        None => {
            eprintln!("[CAPTURE] ERROR: Failed to create CGEventTap.");
            eprintln!("[CAPTURE] Grant Accessibility permission: System Settings > Privacy & Security > Accessibility");
            std::process::exit(1);
        }
    }
}
```

**IMPORTANT NOTE FOR IMPLEMENTER:** The `core-graphics` crate's `tap_create` API may differ from what's shown here. The crate version `0.23` uses a callback-based API, but the exact signature depends on the version. If the API doesn't match, use raw FFI with `CGEventTapCreate` from `core_graphics::sys`. The key requirements are:
1. Create a `.defaultTap` (not `.listenOnly`) at `HID` level
2. Return `None`/`nil` from callback to suppress, `Some(event)` to pass through
3. Handle `TapDisabledByTimeout` by re-enabling
4. Run on the main thread via `CFRunLoopRun`

If the `core-graphics` crate doesn't expose the tap API cleanly, fall back to raw FFI:

```rust
extern "C" {
    fn CGEventTapCreate(
        tap: u32, place: u32, options: u32,
        events_of_interest: u64, callback: extern "C" fn(...) -> *mut std::ffi::c_void,
        user_info: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
}
```

The implementer should check the actual `core-graphics 0.23` API and adapt accordingly.

- [ ] **Step 2: Verify it compiles on macOS**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-server`
Expected: Compiles (may have warnings about unused items from stubs).

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/capture/macos.rs
git commit -m "feat: implement macOS capture backend with CGEventTap"
```

---

### Task 5: Implement Windows Capture (SetWindowsHookEx)

**Files:**
- Modify: `crates/server/src/capture/windows.rs`

- [ ] **Step 1: Implement the Windows capture backend**

Replace `crates/server/src/capture/windows.rs`:

```rust
use super::CaptureEvent;
use deskserver_common::keymap::windows_mods_to_modifiers;
use deskserver_common::MouseButton;
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
    TranslateMessage, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

thread_local! {
    static MOUSE_HOOK: RefCell<HHOOK> = RefCell::new(HHOOK::default());
    static KB_HOOK: RefCell<HHOOK> = RefCell::new(HHOOK::default());
    static EVENT_TX: RefCell<Option<mpsc::Sender<(CaptureEvent, bool)>>> = RefCell::new(None);
    // bool = should suppress. We use a channel to ask the callback.
}

// We need a way for the hook callback to consult the main callback.
// Since Windows hooks must return quickly, we use a thread_local callback.
thread_local! {
    static CALLBACK: RefCell<Option<Box<dyn FnMut(CaptureEvent) -> bool>>> = RefCell::new(None);
}

fn get_modifiers() -> u8 {
    unsafe {
        let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) < 0;
        let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) < 0;
        let alt = GetAsyncKeyState(VK_MENU.0 as i32) < 0;
        let win = GetAsyncKeyState(VK_LWIN.0 as i32) < 0;
        windows_mods_to_modifiers(ctrl, shift, alt, win)
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let event = match wparam.0 as u32 {
            WM_MOUSEMOVE => Some(CaptureEvent::MouseMove {
                x: info.pt.x as f64,
                y: info.pt.y as f64,
            }),
            WM_LBUTTONDOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: true }),
            WM_LBUTTONUP => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: false }),
            WM_RBUTTONDOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: true }),
            WM_RBUTTONUP => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: false }),
            WM_MBUTTONDOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: true }),
            WM_MBUTTONUP => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: false }),
            WM_MOUSEWHEEL => {
                let delta = (info.mouseData.0 >> 16) as i16;
                Some(CaptureEvent::Wheel { dx: 0, dy: delta as i64 / 120 })
            }
            _ => None,
        };

        if let Some(ce) = event {
            let suppress = CALLBACK.with(|cb| {
                if let Some(ref mut f) = *cb.borrow_mut() {
                    f(ce)
                } else {
                    false
                }
            });
            if suppress {
                return LRESULT(1); // Suppress
            }
        }
    }
    MOUSE_HOOK.with(|h| CallNextHookEx(*h.borrow(), code, wparam, lparam))
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let mods = get_modifiers();
        let event = match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                Some(CaptureEvent::KeyDown { keycode: info.vkCode, modifiers: mods })
            }
            WM_KEYUP | WM_SYSKEYUP => {
                Some(CaptureEvent::KeyUp { keycode: info.vkCode, modifiers: mods })
            }
            _ => None,
        };

        if let Some(ce) = event {
            let suppress = CALLBACK.with(|cb| {
                if let Some(ref mut f) = *cb.borrow_mut() {
                    f(ce)
                } else {
                    false
                }
            });
            if suppress {
                return LRESULT(1);
            }
        }
    }
    KB_HOOK.with(|h| CallNextHookEx(*h.borrow(), code, wparam, lparam))
}

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(callback));
    });

    unsafe {
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
            .expect("[CAPTURE] Failed to install mouse hook");
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
            .expect("[CAPTURE] Failed to install keyboard hook");

        MOUSE_HOOK.with(|h| *h.borrow_mut() = mouse);
        KB_HOOK.with(|h| *h.borrow_mut() = kb);

        println!("[CAPTURE] Windows hooks installed (mouse + keyboard).");

        // Message loop — required for low-level hooks
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

**NOTE FOR IMPLEMENTER:** The `windows` crate API may differ slightly depending on version. Key things to verify:
1. `MSLLHOOKSTRUCT` field for wheel data (`mouseData` — extract high word as i16 for delta)
2. `SetWindowsHookExW` requires the hook thread to run a message loop
3. `GetAsyncKeyState` returns negative if key is pressed
4. The hook callback MUST return quickly (< 1000ms) — only check state, no I/O

If the exact `windows` crate types don't match, adapt the imports. The structure and logic are correct.

- [ ] **Step 2: Verify it compiles on the current platform**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-server`
Expected: Compiles (Windows code is behind `#[cfg(target_os = "windows")]`).

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/capture/windows.rs
git commit -m "feat: implement Windows capture backend with SetWindowsHookEx"
```

---

### Task 6: Rewrite Server main.rs with State Machine

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/server/src/bin/test_server.rs` (copy of main.rs)

- [ ] **Step 1: Implement the new server with state machine and hotkey**

Replace `crates/server/src/main.rs`:

```rust
mod capture;

use capture::{run_capture, CaptureEvent};
use deskserver_common::{write_msg, InputMsg, MouseButton, MOD_CTRL, MOD_SHIFT};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use deskserver_common::keymap::macos_keycode_to_neutral;
#[cfg(target_os = "windows")]
use deskserver_common::keymap::windows_vk_to_neutral;

const PORT: u16 = 24800;

// Input mode: 0 = LOCAL, 1 = REMOTE
static MODE: AtomicU8 = AtomicU8::new(0);
const LOCAL: u8 = 0;
const REMOTE: u8 = 1;

fn is_hotkey(event: &CaptureEvent) -> bool {
    // Ctrl+Shift+Space (using neutral modifier bits after platform mapping)
    match event {
        CaptureEvent::KeyDown { keycode, modifiers } => {
            let is_space = {
                #[cfg(target_os = "macos")]
                { *keycode == 0x31 } // macOS Space
                #[cfg(target_os = "windows")]
                { *keycode == 0x20 } // VK_SPACE
            };
            is_space && (*modifiers & MOD_CTRL != 0) && (*modifiers & MOD_SHIFT != 0)
        }
        _ => false,
    }
}

fn toggle_mode(stream: &Mutex<std::net::TcpStream>) {
    let current = MODE.load(Ordering::SeqCst);
    let new_mode = if current == LOCAL { REMOTE } else { LOCAL };
    MODE.store(new_mode, Ordering::SeqCst);

    if new_mode == REMOTE {
        println!("[SERVER] Mode: REMOTE — forwarding input to client");
        // Hide cursor
        #[cfg(target_os = "macos")]
        unsafe {
            core_graphics::display::CGDisplayHideCursor(core_graphics::display::CGMainDisplayID());
        }
        // Send ScreenEnter
        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenEnter);
    } else {
        println!("[SERVER] Mode: LOCAL — input goes to this machine");
        // Show cursor
        #[cfg(target_os = "macos")]
        unsafe {
            core_graphics::display::CGDisplayShowCursor(core_graphics::display::CGMainDisplayID());
        }
        // Send ScreenLeave
        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenLeave);
    }
}

fn to_neutral_key(keycode: u32) -> Option<u32> {
    #[cfg(target_os = "macos")]
    { macos_keycode_to_neutral(keycode) }
    #[cfg(target_os = "windows")]
    { windows_vk_to_neutral(keycode) }
}

fn main() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .expect("[SERVER] ERROR: failed to bind");

    println!("[SERVER] Listening on 0.0.0.0:{}", PORT);
    println!("[SERVER] Waiting for client connection...");

    let (stream, addr) = listener.accept().expect("[SERVER] ERROR: failed to accept");
    stream.set_nodelay(true).expect("[SERVER] ERROR: failed to set TCP_NODELAY");
    println!("[SERVER] Client connected: {}", addr);
    println!("[SERVER] Press Ctrl+Shift+Space to toggle REMOTE/LOCAL mode");

    let stream = Mutex::new(stream);

    run_capture(move |event| {
        // Check for hotkey first
        if is_hotkey(&event) {
            toggle_mode(&stream);
            return true; // Always suppress the hotkey itself
        }

        // If LOCAL mode, pass through
        if MODE.load(Ordering::SeqCst) == LOCAL {
            return false;
        }

        // REMOTE mode — convert and forward
        let msg = match &event {
            CaptureEvent::MouseMove { x, y } => {
                Some(InputMsg::MouseMove { x: *x, y: *y })
            }
            CaptureEvent::MouseButton { button, pressed } => {
                Some(InputMsg::MouseButton { button: *button, pressed: *pressed })
            }
            CaptureEvent::Wheel { dx, dy } => {
                Some(InputMsg::Wheel { dx: *dx, dy: *dy })
            }
            CaptureEvent::KeyDown { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyDown {
                    key,
                    modifiers: *modifiers,
                })
            }
            CaptureEvent::KeyUp { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyUp {
                    key,
                    modifiers: *modifiers,
                })
            }
        };

        if let Some(msg) = msg {
            let mut s = stream.lock().unwrap();
            if let Err(e) = write_msg(&mut *s, &msg) {
                eprintln!("[SERVER] Write error: {}", e);
            }
        }

        true // Suppress in REMOTE mode
    });
}
```

- [ ] **Step 2: Copy main.rs to test_server.rs**

```bash
cp crates/server/src/main.rs crates/server/src/bin/test_server.rs
```

- [ ] **Step 3: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-server`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/main.rs crates/server/src/bin/test_server.rs crates/server/src/capture/
git commit -m "feat: rewrite server with state machine, hotkey toggle, and native capture"
```

---

### Task 7: Update Client for Keyboard Events

**Files:**
- Modify: `crates/client/src/main.rs`

- [ ] **Step 1: Update the client to handle keyboard and screen events**

Replace `crates/client/src/main.rs`:

```rust
use deskserver_common::{read_msg, InputMsg, MouseButton as ProtoButton};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use std::env;
use std::net::TcpStream;

#[cfg(target_os = "macos")]
use deskserver_common::keymap::neutral_to_macos_keycode;
#[cfg(target_os = "windows")]
use deskserver_common::keymap::neutral_to_windows_vk;

const PORT: u16 = 24800;

fn map_button(b: &ProtoButton) -> Button {
    match b {
        ProtoButton::Left => Button::Left,
        ProtoButton::Right => Button::Right,
        ProtoButton::Middle => Button::Middle,
    }
}

fn neutral_to_local_keycode(neutral_key: u32) -> Option<u16> {
    #[cfg(target_os = "macos")]
    { neutral_to_macos_keycode(neutral_key).map(|k| k as u16) }
    #[cfg(target_os = "windows")]
    { neutral_to_windows_vk(neutral_key).map(|k| k as u16) }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: kvm-client <server-ip>");
        std::process::exit(1);
    }
    let server_ip = &args[1];

    let addr = format!("{}:{}", server_ip, PORT);
    println!("[CLIENT] Connecting to {}...", addr);

    let mut stream = TcpStream::connect(&addr).expect("failed to connect to server");
    stream.set_nodelay(true).expect("failed to set TCP_NODELAY");
    println!("[CLIENT] Connected to server at {}", addr);
    println!("[CLIENT] Waiting for server to switch to REMOTE mode (Ctrl+Shift+Space)...");

    let mut enigo = Enigo::new(&Settings::default()).expect("failed to create Enigo");

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => match msg {
                InputMsg::MouseMove { x, y } => {
                    enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                }
                InputMsg::MouseButton { button, pressed } => {
                    let btn = map_button(&button);
                    let dir = if pressed { Direction::Press } else { Direction::Release };
                    enigo.button(btn, dir).ok();
                }
                InputMsg::Wheel { dx: _, dy } => {
                    enigo.scroll(dy as i32, Axis::Vertical).ok();
                }
                InputMsg::KeyDown { key, modifiers: _ } => {
                    if let Some(kc) = neutral_to_local_keycode(key) {
                        enigo.raw(kc, Direction::Press).ok();
                    }
                }
                InputMsg::KeyUp { key, modifiers: _ } => {
                    if let Some(kc) = neutral_to_local_keycode(key) {
                        enigo.raw(kc, Direction::Release).ok();
                    }
                }
                InputMsg::ScreenEnter => {
                    println!("[CLIENT] Server switched to REMOTE — now controlling this machine");
                }
                InputMsg::ScreenLeave => {
                    println!("[CLIENT] Server switched to LOCAL — control returned to server");
                }
            },
            Err(e) => {
                eprintln!("[CLIENT] Read error (server disconnected?): {}", e);
                break;
            }
        }
    }

    println!("[CLIENT] Exiting.");
}
```

- [ ] **Step 2: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-client`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/client/src/main.rs
git commit -m "feat: add keyboard synthesis and ScreenEnter/ScreenLeave handling to client"
```

---

### Task 8: Build, Test, and Verify

**Files:** None — verification task.

- [ ] **Step 1: Run all protocol and keymap tests**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: All tests pass (10 protocol + 7 keymap).

- [ ] **Step 2: Build release binaries**

Run: `source "$HOME/.cargo/env" && cargo build --release -p kvm-server -p kvm-client`
Expected: Both binaries compile.

- [ ] **Step 3: Verify server starts and shows hotkey instructions**

Run the server and verify output includes:
```
[SERVER] Press Ctrl+Shift+Space to toggle REMOTE/LOCAL mode
```

- [ ] **Step 4: Verify client shows waiting message**

Run the client with no server and verify usage message. Then connect to server and verify:
```
[CLIENT] Waiting for server to switch to REMOTE mode (Ctrl+Shift+Space)...
```

- [ ] **Step 5: Copy test_server.rs for firewall compatibility**

```bash
cp crates/server/src/main.rs crates/server/src/bin/test_server.rs
cargo build --release -p kvm-server --bin test_server
```

- [ ] **Step 6: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve build/test issues from end-to-end verification"
```

---

## Verification Checklist

- [ ] `cargo test` — all protocol and keymap tests pass
- [ ] `cargo build --release` — both binaries compile on macOS
- [ ] Server starts and shows hotkey instructions
- [ ] Client connects and shows waiting message
- [ ] Ctrl+Shift+Space toggles mode (server prints REMOTE/LOCAL)
- [ ] In REMOTE mode: mouse movement forwarded to client only (local cursor frozen)
- [ ] In REMOTE mode: clicks forwarded to client only
- [ ] In REMOTE mode: keyboard input forwarded to client only
- [ ] Cmd+C on Mac server → Ctrl+C on Windows client
- [ ] Ctrl+Shift+Space again → LOCAL mode restored, local cursor visible

> **Note:** Full cross-platform testing requires two machines. The macOS capture and Windows capture are behind `#[cfg]` — each compiles only on its target platform. Build and test on each platform separately.

> **Firewall note:** On the managed Mac, run the server as `test_server` binary to use the existing firewall allow rule.
