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
    let fake_len: u32 = 2 * 1024 * 1024;
    let mut buf = Vec::new();
    buf.extend_from_slice(&fake_len.to_le_bytes());
    buf.extend_from_slice(&[0u8; 64]);

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
    let msg = InputMsg::ScreenEnter { x: 150.5, y: 300.0 };
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
