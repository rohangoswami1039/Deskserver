use deskserver_common::{write_msg, InputMsg, MouseButton};
use rdev::{listen, EventType, Button};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

const PORT: u16 = 24800;
static EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

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
    println!("[SERVER] Starting rdev::listen...");
    println!("[SERVER] If you see NO 'event captured' messages, Input Monitoring permission is missing.");
    println!("[SERVER] Grant it at: System Settings > Privacy & Security > Input Monitoring");
    println!("[SERVER] Waiting for mouse events...");
    listen(move |event| {
        let count = EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = match event.event_type {
            EventType::MouseMove { x, y } => {
                if count <= 5 || count % 100 == 0 {
                    println!("[SERVER] Event #{}: MouseMove({:.0}, {:.0})", count, x, y);
                }
                Some(InputMsg::MouseMove { x, y })
            }
            EventType::ButtonPress(b) => {
                println!("[SERVER] Event #{}: ButtonPress({:?})", count, b);
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: true,
                })
            }
            EventType::ButtonRelease(b) => {
                println!("[SERVER] Event #{}: ButtonRelease({:?})", count, b);
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: false,
                })
            }
            EventType::Wheel { delta_x, delta_y } => {
                println!("[SERVER] Event #{}: Wheel({}, {})", count, delta_x, delta_y);
                Some(InputMsg::Wheel {
                    dx: delta_x,
                    dy: delta_y,
                })
            }
            _ => {
                if count <= 5 {
                    println!("[SERVER] Event #{}: Other({:?})", count, event.event_type);
                }
                None
            }
        };

        if let Some(msg) = msg {
            if tx.send(msg).is_err() {
                // Writer thread exited (client disconnected) — stop silently
                return;
            }
        }
    })
    .expect("[SERVER] ERROR: failed to start event listener (permission denied?)");
}
