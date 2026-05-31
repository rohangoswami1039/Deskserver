use deskserver_common::keymap::*;

#[test]
fn neutral_a_roundtrips_macos() {
    let neutral = macos_keycode_to_neutral(0x00);
    assert_eq!(neutral, Some(NK_A));
    let back = neutral_to_macos_keycode(NK_A);
    assert_eq!(back, Some(0x00));
}

#[test]
fn neutral_a_roundtrips_windows() {
    let neutral = windows_vk_to_neutral(0x41);
    assert_eq!(neutral, Some(NK_A));
    let back = neutral_to_windows_vk(NK_A);
    assert_eq!(back, Some(0x41));
}

#[test]
fn cmd_ctrl_swap_mac_to_win() {
    let neutral = macos_keycode_to_neutral(0x37); // Left Cmd on macOS
    assert_eq!(neutral, Some(NK_CTRL_LEFT));
    let vk = neutral_to_windows_vk(NK_CTRL_LEFT);
    assert_eq!(vk, Some(0xA2)); // VK_LCONTROL
}

#[test]
fn cmd_ctrl_swap_win_to_mac() {
    let neutral = windows_vk_to_neutral(0xA2); // VK_LCONTROL
    assert_eq!(neutral, Some(NK_CTRL_LEFT));
    let mac_kc = neutral_to_macos_keycode(NK_CTRL_LEFT);
    assert_eq!(mac_kc, Some(0x37)); // Left Cmd on macOS
}

#[test]
fn modifier_bits_from_macos() {
    use deskserver_common::{MOD_CTRL, MOD_SHIFT};
    let mods = macos_flags_to_modifiers(0x120108); // Cmd+Shift flags
    assert_eq!(mods & MOD_CTRL, MOD_CTRL);
    assert_eq!(mods & MOD_SHIFT, MOD_SHIFT);
}

#[test]
fn modifier_bits_from_windows() {
    use deskserver_common::MOD_CTRL;
    let mods = windows_mods_to_modifiers(true, false, false, false);
    assert_eq!(mods & MOD_CTRL, MOD_CTRL);
}

#[test]
fn space_key_maps() {
    let neutral = macos_keycode_to_neutral(0x31);
    assert_eq!(neutral, Some(NK_SPACE));
    let vk = neutral_to_windows_vk(NK_SPACE);
    assert_eq!(vk, Some(0x20));
}
