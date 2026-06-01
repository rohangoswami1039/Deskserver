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

use std::cell::RefCell;
thread_local! {
    static LAST_LSHIFT_TAP: RefCell<Option<Instant>> = RefCell::new(None);
}

fn is_hotkey(event: &CaptureEvent) -> bool {
    match event {
        CaptureEvent::KeyUp { keycode, .. } => {
            let is_left_shift = {
                #[cfg(target_os = "macos")]
                { *keycode == 0x38 }
                #[cfg(target_os = "windows")]
                { *keycode == 0xA0 }
            };
            if !is_left_shift {
                return false;
            }
            LAST_LSHIFT_TAP.with(|last| {
                let now = Instant::now();
                let mut last = last.borrow_mut();
                if let Some(prev) = *last {
                    if now.duration_since(prev).as_millis() < 400 {
                        *last = None;
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

    // Connection test
    {
        let mut s = &stream;
        let test_msgs: Vec<(&str, InputMsg)> = vec![
            ("MouseMove(100, 100)", InputMsg::MouseMove { x: 100.0, y: 100.0 }),
            ("MouseButton(Left, press)", InputMsg::MouseButton { button: MouseButton::Left, pressed: true }),
            ("MouseButton(Left, release)", InputMsg::MouseButton { button: MouseButton::Left, pressed: false }),
        ];
        println!("[SERVER] Sending {} test messages...", test_msgs.len());
        for (i, (label, msg)) in test_msgs.iter().enumerate() {
            match write_msg(&mut s, msg) {
                Ok(()) => println!("[SERVER] Test {}/{}: {} — OK", i + 1, test_msgs.len(), label),
                Err(e) => {
                    eprintln!("[SERVER] ERROR: {} — {}", label, e);
                    std::process::exit(1);
                }
            }
            thread::sleep(Duration::from_millis(300));
        }
        println!("[SERVER] Connection verified!");
    }

    println!("[SERVER] Double-tap Left Shift to toggle | Move cursor to left edge to cross");

    let stream = Mutex::new(stream);

    // Auto-detect screen resolution
    let (server_width, server_height) = {
        #[cfg(target_os = "macos")]
        {
            let (w, h) = kvm_server_lib::capture::macos::get_screen_center();
            (w * 2.0, h * 2.0)
        }
        #[cfg(target_os = "windows")]
        { (1920.0_f64, 1080.0_f64) }
    };
    let client_width: f64 = 1920.0;
    let client_height: f64 = 1080.0;
    println!("[SERVER] Screen: {:.0}x{:.0}", server_width, server_height);

    let mut virtual_x: f64 = 0.0;
    let mut virtual_y: f64 = 0.0;
    let mut saved_x: f64 = 0.0;
    let mut saved_y: f64 = 0.0;
    let mut hide_count: i32 = 0; // Track hide/show calls for proper cleanup

    // Helper closures for enter/exit REMOTE
    let enter_remote = |stream: &Mutex<std::net::TcpStream>,
                        virtual_x: &mut f64, virtual_y: &mut f64,
                        saved_x: &mut f64, saved_y: &mut f64,
                        hide_count: &mut i32,
                        cursor_x: f64, cursor_y: f64,
                        entry_x: f64, entry_y: f64| {
        MODE.store(REMOTE, Ordering::SeqCst);
        *virtual_x = entry_x;
        *virtual_y = entry_y;
        *saved_x = cursor_x;
        *saved_y = cursor_y;

        println!("[SERVER] → REMOTE (entry {:.0},{:.0}) saved Mac ({:.0},{:.0})", entry_x, entry_y, cursor_x, cursor_y);

        #[cfg(target_os = "macos")]
        {
            kvm_server_lib::capture::macos::hide_cursor();
            *hide_count += 1;
        }

        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenEnter { x: entry_x, y: entry_y });
    };

    let exit_remote = |stream: &Mutex<std::net::TcpStream>,
                       saved_x: f64, saved_y: f64,
                       hide_count: &mut i32| {
        MODE.store(LOCAL, Ordering::SeqCst);

        println!("[SERVER] → LOCAL (restore {:.0},{:.0})", saved_x, saved_y);

        #[cfg(target_os = "macos")]
        {
            // Warp cursor to saved position FIRST, then show
            kvm_server_lib::capture::macos::warp_cursor_to(saved_x, saved_y);
            // Show cursor — undo all hides
            while *hide_count > 0 {
                kvm_server_lib::capture::macos::show_cursor();
                *hide_count -= 1;
            }
            // Extra show to be safe
            kvm_server_lib::capture::macos::show_cursor();
        }

        let mut s = stream.lock().unwrap();
        let _ = write_msg(&mut *s, &InputMsg::ScreenLeave);
    };

    run_capture(move |event| {
        // Hotkey toggle
        if is_hotkey(&event) {
            if MODE.load(Ordering::SeqCst) == LOCAL {
                enter_remote(&stream, &mut virtual_x, &mut virtual_y,
                    &mut saved_x, &mut saved_y, &mut hide_count,
                    server_width / 2.0, server_height / 2.0,
                    client_width / 2.0, client_height / 2.0);
            } else {
                exit_remote(&stream, saved_x, saved_y, &mut hide_count);
            }
            return true;
        }

        if MODE.load(Ordering::SeqCst) == LOCAL {
            // Edge crossing check — Windows is on the LEFT
            if let CaptureEvent::MouseMove { x, y, .. } = &event {
                if *x <= 1.0 {
                    let pct = *y / server_height;
                    let entry_y = pct * client_height;
                    let entry_x = client_width - 1.0;

                    enter_remote(&stream, &mut virtual_x, &mut virtual_y,
                        &mut saved_x, &mut saved_y, &mut hide_count,
                        *x, *y, entry_x, entry_y);
                    return true;
                }
            }
            return false; // Pass through in LOCAL
        }

        // ═══════════════════════════════════════════
        // REMOTE MODE
        // ═══════════════════════════════════════════
        // Strategy: read raw hardware deltas (unaffected by cursor warps),
        // forward to client, then warp Mac cursor back to saved position.
        // CGWarpMouseCursorPosition does NOT generate new mouse events (Apple docs),
        // so no event skipping needed. The delta fields (kCGMouseEventDeltaX/Y)
        // reflect physical mouse movement, not cursor position changes.

        let msg = match &event {
            CaptureEvent::MouseMove { delta_x, delta_y, .. } => {
                // Raw hardware deltas — not affected by our warps
                let dx = *delta_x;
                let dy = *delta_y;

                // Ignore zero-deltas (system noise or warp artifacts if any)
                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                    return true;
                }

                virtual_x += dx;
                virtual_y += dy;

                // Return crossing: virtual cursor past RIGHT edge → back to Mac
                if virtual_x > client_width {
                    exit_remote(&stream, saved_x, saved_y, &mut hide_count);
                    return true;
                }

                // Clamp
                virtual_x = virtual_x.clamp(0.0, client_width);
                virtual_y = virtual_y.clamp(0.0, client_height);

                // Warp Mac cursor back to saved position to keep it visually frozen.
                // This does NOT generate new events (Apple docs: CGWarpMouseCursorPosition
                // "does not generate or post an event to account for the new position").
                #[cfg(target_os = "macos")]
                kvm_server_lib::capture::macos::warp_cursor_to(saved_x, saved_y);

                Some(InputMsg::MouseMove { x: dx, y: dy })
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
