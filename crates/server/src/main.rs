use kvm_server_lib::capture::{run_capture, CaptureEvent};
use deskserver_common::{write_msg, InputMsg};
use std::time::Instant;
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

// Double-tap Right Shift to toggle REMOTE/LOCAL
use std::cell::RefCell;
thread_local! {
    static LAST_RSHIFT_TAP: RefCell<Option<Instant>> = RefCell::new(None);
}

fn is_hotkey(event: &CaptureEvent) -> bool {
    // Detect Left Shift key-up (release) — double-tap within 400ms
    match event {
        CaptureEvent::KeyUp { keycode, .. } => {
            let is_left_shift = {
                #[cfg(target_os = "macos")]
                { *keycode == 0x38 } // Left Shift on macOS
                #[cfg(target_os = "windows")]
                { *keycode == 0xA0 } // VK_LSHIFT on Windows
            };
            if !is_left_shift {
                return false;
            }
            LAST_RSHIFT_TAP.with(|last| {
                let now = Instant::now();
                let mut last = last.borrow_mut();
                if let Some(prev) = *last {
                    if now.duration_since(prev).as_millis() < 400 {
                        *last = None; // Reset so triple-tap doesn't re-trigger
                        return true;
                    }
                }
                *last = Some(now);
                false
            })
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
        {
            kvm_server_lib::capture::macos::hide_cursor();
            kvm_server_lib::capture::macos::disconnect_mouse();
        }
        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenEnter { x: 0.0, y: 0.0 });
    } else {
        println!("[SERVER] Mode: LOCAL — input goes to this machine");
        #[cfg(target_os = "macos")]
        {
            kvm_server_lib::capture::macos::reconnect_mouse();
            kvm_server_lib::capture::macos::show_cursor();
            kvm_server_lib::capture::macos::show_cursor();
            kvm_server_lib::capture::macos::show_cursor();
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

    println!("[SERVER] Double-tap Left Shift to toggle REMOTE/LOCAL mode");

    let stream = Mutex::new(stream);

    // Auto-detect server screen resolution
    let (server_width, server_height) = {
        #[cfg(target_os = "macos")]
        {
            let (w, h) = kvm_server_lib::capture::macos::get_screen_center();
            (w * 2.0, h * 2.0) // get_screen_center returns center, so double for full size
        }
        #[cfg(target_os = "windows")]
        { (1920.0_f64, 1080.0_f64) }
    };
    let client_width: f64 = 1920.0;
    let client_height: f64 = 1080.0;
    println!("[SERVER] Screen size: {:.0}x{:.0}", server_width, server_height);

    let mut virtual_x: f64 = 0.0;
    let mut virtual_y: f64 = 0.0;
    let mut skip_deltas: u8 = 0;
    let mut saved_cursor_x: f64 = 0.0;
    let mut saved_cursor_y: f64 = 0.0;

    run_capture(move |event| {
        if is_hotkey(&event) {
            toggle_mode(&stream);
            return true; // Always suppress the hotkey itself
        }

        if MODE.load(Ordering::SeqCst) == LOCAL {
            // Skip transition events after returning from REMOTE
            if skip_deltas > 0 {
                if matches!(&event, CaptureEvent::MouseMove { .. }) {
                    skip_deltas -= 1;
                }
                return false; // Pass through but don't check edges yet
            }

            // Check for edge crossing — Windows is on the LEFT of Mac
            if let CaptureEvent::MouseMove { x, y, .. } = &event {
                // Debug: log cursor position near edges
                if *x <= 5.0 || *x >= server_width - 5.0 {
                    println!("[SERVER] Cursor near edge: x={:.0}, y={:.0} (screen={:.0}x{:.0})", x, y, server_width, server_height);
                }

                if *x <= 2.0 {
                    // Hit LEFT edge — cross to remote (Windows is on the left)
                    let pct = *y / server_height;
                    let entry_y = pct * client_height;
                    let entry_x = client_width; // Enter from RIGHT edge of Windows

                    MODE.store(REMOTE, Ordering::SeqCst);
                    virtual_x = entry_x;
                    virtual_y = entry_y;
                    saved_cursor_x = *x;
                    saved_cursor_y = *y;
                    skip_deltas = 2;

                    println!("[SERVER] Edge crossing → REMOTE (entry at {:.0}, {:.0}), saved Mac pos ({:.0}, {:.0})", entry_x, entry_y, saved_cursor_x, saved_cursor_y);

                    #[cfg(target_os = "macos")]
                    {
                        kvm_server_lib::capture::macos::hide_cursor();
                        kvm_server_lib::capture::macos::disconnect_mouse();
                    }

                    let mut s = stream.lock().unwrap();
                    let _ = write_msg(&mut *s, &InputMsg::ScreenEnter { x: entry_x, y: entry_y });
                    return true;
                }
            }
            return false;
        }

        // REMOTE mode — cursor hidden + disconnected, just read raw deltas
        let msg = match &event {
            CaptureEvent::MouseMove { delta_x, delta_y, .. } => {
                // Skip initial events after crossing
                if skip_deltas > 0 {
                    skip_deltas -= 1;
                    return true;
                }

                virtual_x += *delta_x;
                virtual_y += *delta_y;

                // Check for return crossing (past RIGHT edge of Windows — back to Mac)
                if virtual_x > client_width {
                    MODE.store(LOCAL, Ordering::SeqCst);
                    skip_deltas = 2;

                    println!("[SERVER] Return crossing → LOCAL");

                    #[cfg(target_os = "macos")]
                    {
                        kvm_server_lib::capture::macos::reconnect_mouse();
                        kvm_server_lib::capture::macos::warp_cursor_to(saved_cursor_x, saved_cursor_y);
                        kvm_server_lib::capture::macos::show_cursor();
                        kvm_server_lib::capture::macos::show_cursor();
                        kvm_server_lib::capture::macos::show_cursor();
                    }

                    let mut s = stream.lock().unwrap();
                    let _ = write_msg(&mut *s, &InputMsg::ScreenLeave);
                    return true;
                }

                // Clamp virtual cursor
                virtual_x = virtual_x.clamp(0.0, client_width);
                virtual_y = virtual_y.clamp(0.0, client_height);

                if delta_x.abs() > 0.1 || delta_y.abs() > 0.1 {
                    Some(InputMsg::MouseMove { x: *delta_x, y: *delta_y })
                } else {
                    None
                }
            }
            CaptureEvent::MouseButton { button, pressed } => {
                Some(InputMsg::MouseButton { button: *button, pressed: *pressed })
            }
            CaptureEvent::Wheel { dx, dy } => {
                Some(InputMsg::Wheel { dx: *dx, dy: *dy })
            }
            CaptureEvent::KeyDown { keycode, modifiers } => {
                let neutral = to_neutral_key(*keycode);
                println!("[SERVER] KeyDown: raw=0x{:02X} ({}) → neutral={:?} mods=0x{:02X}", keycode, keycode, neutral, modifiers);
                neutral.map(|key| InputMsg::KeyDown {
                    key,
                    modifiers: *modifiers,
                })
            }
            CaptureEvent::KeyUp { keycode, modifiers } => {
                let neutral = to_neutral_key(*keycode);
                neutral.map(|key| InputMsg::KeyUp {
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
