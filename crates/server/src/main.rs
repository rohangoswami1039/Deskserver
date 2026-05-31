use deskserver_common::{write_msg, InputMsg, MouseButton};
use rdev::{listen, EventType, Button};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

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
        .expect("[SERVER] ERROR: failed to bind");

    println!("[SERVER] Listening on 0.0.0.0:{}", PORT);
    println!("[SERVER] Waiting for client connection...");

    let (stream, addr) = listener.accept().expect("[SERVER] ERROR: failed to accept");
    stream.set_nodelay(true).expect("[SERVER] ERROR: failed to set TCP_NODELAY");
    println!("[SERVER] Client connected: {}", addr);

    // Wrap stream in Mutex for use inside rdev callback
    let stream = Mutex::new(stream);

    // Send handshake to verify pipe is alive
    {
        let mut s = stream.lock().unwrap();
        match write_msg(&mut *s, &InputMsg::MouseMove { x: 0.0, y: 0.0 }) {
            Ok(()) => println!("[SERVER] Handshake sent OK"),
            Err(e) => {
                eprintln!("[SERVER] ERROR: Handshake failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    println!("[SERVER] Starting rdev::listen...");
    println!("[SERVER] Move the mouse to see events. Ctrl+C to stop.");

    listen(move |event| {
        let count = EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = match event.event_type {
            EventType::MouseMove { x, y } => {
                if count <= 5 || count % 200 == 0 {
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
            _ => None,
        };

        if let Some(msg) = msg {
            let mut s = stream.lock().unwrap();
            if let Err(e) = write_msg(&mut *s, &msg) {
                eprintln!("[SERVER] Write error at event #{}: {}", count, e);
            }
        }
    })
    .expect("[SERVER] ERROR: failed to start event listener");
}
