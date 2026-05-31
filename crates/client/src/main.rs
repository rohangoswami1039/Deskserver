use deskserver_common::{read_msg, InputMsg, MouseButton as ProtoButton};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};
use std::env;
use std::net::TcpStream;

const PORT: u16 = 24800;

fn map_button(b: &ProtoButton) -> Button {
    match b {
        ProtoButton::Left => Button::Left,
        ProtoButton::Right => Button::Right,
        ProtoButton::Middle => Button::Middle,
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

    let mut enigo = Enigo::new(&Settings::default()).expect("failed to create Enigo");

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => match msg {
                InputMsg::MouseMove { x, y } => {
                    enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                }
                InputMsg::MouseButton { button, pressed } => {
                    let btn = map_button(&button);
                    let dir = if pressed {
                        Direction::Press
                    } else {
                        Direction::Release
                    };
                    enigo.button(btn, dir).ok();
                }
                InputMsg::Wheel { dx: _, dy } => {
                    enigo.scroll(dy as i32, Axis::Vertical).ok();
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
