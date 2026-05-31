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
pub const NK_CTRL_LEFT: u32 = 92;
pub const NK_CTRL_RIGHT: u32 = 93;
pub const NK_ALT_LEFT: u32 = 94;
pub const NK_ALT_RIGHT: u32 = 95;
pub const NK_META_LEFT: u32 = 96;
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
