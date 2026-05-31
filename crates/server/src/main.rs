use deskserver_common::{write_msg, InputMsg, MouseButton};
use rdev::{listen, EventType, Button};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

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

    // Phase 1: Send 5 test messages (1/sec) to prove the pipe works
    {
        let mut s = &stream;
        for i in 1..=5 {
            let msg = InputMsg::MouseMove { x: (i * 100) as f64, y: (i * 100) as f64 };
            println!("[SERVER] Test message {}/5: MouseMove({}, {})", i, i*100, i*100);
            match write_msg(&mut s, &msg) {
                Ok(()) => println!("[SERVER] Test message {}/5 sent OK", i),
                Err(e) => {
                    eprintln!("[SERVER] ERROR: Test message {} failed: {}", i, e);
                    eprintln!("[SERVER] Client disconnected during test phase. Exiting.");
                    std::process::exit(1);
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        println!("[SERVER] All 5 test messages sent! Pipe is verified.");
    }

    // Phase 2: Switch to rdev capture
    println!("[SERVER] Now switching to live mouse capture...");
    let stream = Mutex::new(stream);

    listen(move |event| {
        let count = EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = match event.event_type {
            EventType::MouseMove { x, y } => {
                if count <= 5 || count % 200 == 0 {
                    println!("[SERVER] Live #{}: MouseMove({:.0}, {:.0})", count, x, y);
                }
                Some(InputMsg::MouseMove { x, y })
            }
            EventType::ButtonPress(b) => {
                println!("[SERVER] Live #{}: ButtonPress({:?})", count, b);
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: true,
                })
            }
            EventType::ButtonRelease(b) => {
                println!("[SERVER] Live #{}: ButtonRelease({:?})", count, b);
                map_button(b).map(|button| InputMsg::MouseButton {
                    button,
                    pressed: false,
                })
            }
            EventType::Wheel { delta_x, delta_y } => {
                println!("[SERVER] Live #{}: Wheel({}, {})", count, delta_x, delta_y);
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
                eprintln!("[SERVER] Write error at live #{}: {}", count, e);
            } else if count <= 3 {
                println!("[SERVER] Live #{} sent OK", count);
            }
        }
    })
    .expect("[SERVER] ERROR: failed to start event listener");
}
