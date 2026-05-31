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
    println!("[SERVER] Client connected: {}", addr);

    // Handshake: send a test message immediately to verify the pipe
    println!("[SERVER] Sending handshake...");
    match write_msg(&mut stream, &InputMsg::MouseMove { x: 0.0, y: 0.0 }) {
        Ok(()) => println!("[SERVER] Handshake sent OK — pipe is alive"),
        Err(e) => {
            eprintln!("[SERVER] ERROR: Handshake failed — client already disconnected: {}", e);
            eprintln!("[SERVER] This usually means a stale client connected. Try again.");
            std::process::exit(1);
        }
    }

    let (tx, rx) = mpsc::channel::<InputMsg>();

    // Writer thread: drain channel → TCP
    thread::spawn(move || {
        let mut write_count: u64 = 0;
        loop {
            match rx.recv() {
                Ok(msg) => {
                    write_count += 1;
                    if let Err(e) = write_msg(&mut stream, &msg) {
                        eprintln!("[SERVER] Write error after {} messages: {}", write_count, e);
                        break;
                    }
                    if write_count <= 3 {
                        println!("[SERVER] Sent message #{} to client", write_count);
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
        println!("[SERVER] Writer thread exiting");
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
