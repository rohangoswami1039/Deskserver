use kvm_server_lib::capture::{run_capture, CaptureEvent};
use deskserver_common::{write_msg, InputMsg, MOD_CTRL};
use std::sync::atomic::AtomicBool;

static SPACE_HELD: AtomicBool = AtomicBool::new(false);
use std::net::TcpListener;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use deskserver_common::keymap::macos_keycode_to_neutral;
#[cfg(target_os = "windows")]
use deskserver_common::keymap::windows_vk_to_neutral;

const PORT: u16 = 24800;

const LOCAL: u8 = 0;
const REMOTE: u8 = 1;
static MODE: AtomicU8 = AtomicU8::new(LOCAL);

fn is_space(keycode: u32) -> bool {
    #[cfg(target_os = "macos")]
    { keycode == 0x31 }
    #[cfg(target_os = "windows")]
    { keycode == 0x20 }
}

fn is_q(keycode: u32) -> bool {
    #[cfg(target_os = "macos")]
    { keycode == 0x0C }
    #[cfg(target_os = "windows")]
    { keycode == 0x51 }
}

fn track_space(event: &CaptureEvent) {
    match event {
        CaptureEvent::KeyDown { keycode, .. } if is_space(*keycode) => {
            SPACE_HELD.store(true, Ordering::SeqCst);
        }
        CaptureEvent::KeyUp { keycode, .. } if is_space(*keycode) => {
            SPACE_HELD.store(false, Ordering::SeqCst);
        }
        _ => {}
    }
}

fn is_hotkey(event: &CaptureEvent) -> bool {
    match event {
        CaptureEvent::KeyDown { keycode, modifiers } => {
            is_q(*keycode)
                && (*modifiers & MOD_CTRL != 0)
                && SPACE_HELD.load(Ordering::SeqCst)
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
    println!("[SERVER] Press Ctrl+Shift+Space to toggle REMOTE/LOCAL mode");

    let stream = Mutex::new(stream);

    run_capture(move |event| {
        track_space(&event);

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
