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
