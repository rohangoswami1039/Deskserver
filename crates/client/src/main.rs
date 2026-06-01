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
    println!("[CLIENT] Waiting for test messages and server commands...");

    let mut enigo = Enigo::new(&Settings::default()).expect("failed to create Enigo");
    let mut msg_count: u64 = 0;
    let mut remote_mode = false;

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => {
                msg_count += 1;
                match msg {
                    InputMsg::MouseMove { x, y } => {
                        if remote_mode {
                            // In REMOTE mode, x/y are deltas from server
                            if msg_count <= 10 {
                                println!("[CLIENT] #{}: MouseMove delta({:.0}, {:.0}) — relative move", msg_count, x, y);
                            }
                            enigo.move_mouse(x as i32, y as i32, Coordinate::Rel).ok();
                        } else {
                            // Test messages use absolute coords
                            if msg_count <= 10 {
                                println!("[CLIENT] #{}: MouseMove({:.0}, {:.0}) — absolute move", msg_count, x, y);
                            }
                            enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                        }
                    }
                    InputMsg::MouseButton { button, pressed } => {
                        let action = if pressed { "pressing" } else { "releasing" };
                        println!("[CLIENT] #{}: MouseButton {:?} — {}", msg_count, button, action);
                        let btn = map_button(&button);
                        let dir = if pressed { Direction::Press } else { Direction::Release };
                        enigo.button(btn, dir).ok();
                    }
                    InputMsg::Wheel { dx: _, dy } => {
                        println!("[CLIENT] #{}: Wheel dy={} — scrolling", msg_count, dy);
                        enigo.scroll(dy as i32, Axis::Vertical).ok();
                    }
                    InputMsg::KeyDown { key, modifiers } => {
                        if let Some(kc) = neutral_to_local_keycode(key) {
                            println!("[CLIENT] #{}: KeyDown key={} (local=0x{:02X}) mods=0x{:02X} — pressing", msg_count, key, kc, modifiers);
                            enigo.raw(kc, Direction::Press).ok();
                        } else {
                            println!("[CLIENT] #{}: KeyDown key={} — unmapped, skipping", msg_count, key);
                        }
                    }
                    InputMsg::KeyUp { key, modifiers } => {
                        if let Some(kc) = neutral_to_local_keycode(key) {
                            println!("[CLIENT] #{}: KeyUp key={} (local=0x{:02X}) mods=0x{:02X} — releasing", msg_count, key, kc, modifiers);
                            enigo.raw(kc, Direction::Release).ok();
                        } else {
                            println!("[CLIENT] #{}: KeyUp key={} — unmapped, skipping", msg_count, key);
                        }
                    }
                    InputMsg::ScreenEnter { x, y } => {
                        remote_mode = true;
                        println!("[CLIENT] #{}: ScreenEnter at ({:.0}, {:.0}) — now controlling this machine", msg_count, x, y);
                        enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                    }
                    InputMsg::ScreenLeave => {
                        remote_mode = false;
                        println!("[CLIENT] #{}: ScreenLeave — server switched to LOCAL, control returned", msg_count);
                    }
                }
            }
            Err(e) => {
                eprintln!("[CLIENT] Read error after {} messages: {}", msg_count, e);
                break;
            }
        }
    }

    println!("[CLIENT] Exiting.");
}
