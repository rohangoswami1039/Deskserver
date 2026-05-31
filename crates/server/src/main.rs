use kvm_server_lib::capture::{run_capture, CaptureEvent};
use deskserver_common::{write_msg, InputMsg, MOD_SHIFT, MOD_META};
use deskserver_common::MouseButton;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use deskserver_common::keymap::macos_keycode_to_neutral;
#[cfg(target_os = "windows")]
use deskserver_common::keymap::windows_vk_to_neutral;

const PORT: u16 = 24800;

const LOCAL: u8 = 0;
const REMOTE: u8 = 1;
static MODE: AtomicU8 = AtomicU8::new(LOCAL);

fn is_hotkey(event: &CaptureEvent) -> bool {
    match event {
        CaptureEvent::KeyDown { keycode, modifiers } => {
            let is_1 = {
                #[cfg(target_os = "macos")]
                { *keycode == 0x12 } // macOS '1' keycode
                #[cfg(target_os = "windows")]
                { *keycode == 0x31 } // VK_1
            };
            // Physical Ctrl+Shift+1
            // On macOS: physical Ctrl → MOD_META (due to Cmd↔Ctrl swap), Shift → MOD_SHIFT
            // On Windows: physical Ctrl → MOD_CTRL, Shift → MOD_SHIFT
            let ctrl_held = {
                #[cfg(target_os = "macos")]
                { *modifiers & MOD_META != 0 } // physical Ctrl is META after swap
                #[cfg(target_os = "windows")]
                { *modifiers & deskserver_common::MOD_CTRL != 0 }
            };
            is_1 && ctrl_held && (*modifiers & MOD_SHIFT != 0)
        }
        _ => false,
    }
}

fn toggle_mode(stream: &Mutex<std::net::TcpStream>) {
    let current = MODE.load(Ordering::SeqCst);
    let new_mode = if current == LOCAL { REMOTE } else { LOCAL };
    MODE.store(new_mode, Ordering::SeqCst);

    if new_mode == REMOTE {
        println!("[SERVER] Mode: REMOTE — forwarding input to client");
        #[cfg(target_os = "macos")]
        unsafe {
            core_graphics::display::CGDisplayHideCursor(core_graphics::display::CGMainDisplayID());
        }
        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenEnter);
    } else {
        println!("[SERVER] Mode: LOCAL — input goes to this machine");
        #[cfg(target_os = "macos")]
        unsafe {
            core_graphics::display::CGDisplayShowCursor(core_graphics::display::CGMainDisplayID());
        }
        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenLeave);
    }
}

fn to_neutral_key(keycode: u32) -> Option<u32> {
    #[cfg(target_os = "macos")]
    { macos_keycode_to_neutral(keycode) }
    #[cfg(target_os = "windows")]
    { windows_vk_to_neutral(keycode) }
}

fn main() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .expect("[SERVER] ERROR: failed to bind");

    println!("[SERVER] Listening on 0.0.0.0:{}", PORT);
    println!("[SERVER] Waiting for client connection...");

    let (stream, addr) = listener.accept().expect("[SERVER] ERROR: failed to accept");
    stream.set_nodelay(true).expect("[SERVER] ERROR: failed to set TCP_NODELAY");
    println!("[SERVER] Client connected: {}", addr);

    // Connection test: send 3 test messages to verify the pipe
    {
        let mut s = &stream;
        let test_msgs: Vec<(&str, InputMsg)> = vec![
            ("MouseMove(100, 100)", InputMsg::MouseMove { x: 100.0, y: 100.0 }),
            ("MouseButton(Left, press)", InputMsg::MouseButton { button: MouseButton::Left, pressed: true }),
            ("MouseButton(Left, release)", InputMsg::MouseButton { button: MouseButton::Left, pressed: false }),
        ];
        println!("[SERVER] Sending {} test messages to verify connection...", test_msgs.len());
        for (i, (label, msg)) in test_msgs.iter().enumerate() {
            match write_msg(&mut s, msg) {
                Ok(()) => println!("[SERVER] Test {}/{}: {} — OK", i + 1, test_msgs.len(), label),
                Err(e) => {
                    eprintln!("[SERVER] ERROR: Test message failed: {} — {}", label, e);
                    eprintln!("[SERVER] Client disconnected. Exiting.");
                    std::process::exit(1);
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        println!("[SERVER] Connection verified!");
    }

    println!("[SERVER] Press Ctrl+Shift+1 to toggle REMOTE/LOCAL mode");

    let stream = Mutex::new(stream);

    run_capture(move |event| {
        if is_hotkey(&event) {
            toggle_mode(&stream);
            return true; // Always suppress the hotkey itself
        }

        if MODE.load(Ordering::SeqCst) == LOCAL {
            return false;
        }

        // REMOTE mode — convert and forward
        let msg = match &event {
            CaptureEvent::MouseMove { x, y } => {
                Some(InputMsg::MouseMove { x: *x, y: *y })
            }
            CaptureEvent::MouseButton { button, pressed } => {
                Some(InputMsg::MouseButton { button: *button, pressed: *pressed })
            }
            CaptureEvent::Wheel { dx, dy } => {
                Some(InputMsg::Wheel { dx: *dx, dy: *dy })
            }
            CaptureEvent::KeyDown { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyDown {
                    key,
                    modifiers: *modifiers,
                })
            }
            CaptureEvent::KeyUp { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyUp {
                    key,
                    modifiers: *modifiers,
                })
            }
        };

        if let Some(msg) = msg {
            let mut s = stream.lock().unwrap();
            if let Err(e) = write_msg(&mut *s, &msg) {
                eprintln!("[SERVER] Write error: {}", e);
            }
        }

        true // Suppress in REMOTE mode
    });
}
