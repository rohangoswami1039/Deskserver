use deskserver_common::{write_msg, InputMsg, MouseButton};
use std::io::Write;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

const PORT: u16 = 24800;

fn main() {
    println!("[SERVER] Starting test server...");
    println!("[SERVER] Binding to 0.0.0.0:{}", PORT);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .expect("[SERVER] ERROR: failed to bind");
    println!("[SERVER] Bound successfully. Waiting for client...");

    let (mut stream, addr) = listener.accept().expect("[SERVER] ERROR: failed to accept");
    stream.set_nodelay(true).expect("[SERVER] ERROR: failed to set TCP_NODELAY");
    println!("[SERVER] Client connected from: {}", addr);

    // Send a sequence of test messages with logging
    let test_messages = vec![
        ("MouseMove(100, 200)", InputMsg::MouseMove { x: 100.0, y: 200.0 }),
        ("MouseMove(300, 400)", InputMsg::MouseMove { x: 300.0, y: 400.0 }),
        ("MouseButton(Left, press)", InputMsg::MouseButton { button: MouseButton::Left, pressed: true }),
        ("MouseButton(Left, release)", InputMsg::MouseButton { button: MouseButton::Left, pressed: false }),
        ("Wheel(0, 3)", InputMsg::Wheel { dx: 0, dy: 3 }),
        ("MouseMove(500, 500)", InputMsg::MouseMove { x: 500.0, y: 500.0 }),
    ];

    println!("[SERVER] Sending {} test messages (1 per second)...", test_messages.len());

    for (i, (label, msg)) in test_messages.iter().enumerate() {
        println!("[SERVER] Sending message {}/{}: {}", i + 1, test_messages.len(), label);
        match write_msg(&mut stream, msg) {
            Ok(()) => println!("[SERVER] Message {}/{} sent OK", i + 1, test_messages.len()),
            Err(e) => {
                eprintln!("[SERVER] ERROR sending message {}: {}", i + 1, e);
                return;
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    println!("[SERVER] All test messages sent successfully!");
    println!("[SERVER] Keeping connection open for 3 more seconds...");
    thread::sleep(Duration::from_secs(3));
    println!("[SERVER] Done. Exiting.");
}
