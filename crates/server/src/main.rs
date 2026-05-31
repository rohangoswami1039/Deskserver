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
