use deskserver_common::MouseButton;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Internal capture event — platform-neutral representation of a raw OS event.
#[derive(Debug, Clone)]
pub enum CaptureEvent {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
    KeyDown { keycode: u32, modifiers: u8 },
    KeyUp { keycode: u32, modifiers: u8 },
}

/// Run the platform-specific capture loop on the current (main) thread.
/// The callback receives each event and returns `true` to suppress it, `false` to pass through.
/// This function blocks forever (runs the OS event loop).
pub fn run_capture<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    #[cfg(target_os = "macos")]
    macos::run(callback);

    #[cfg(target_os = "windows")]
    windows::run(callback);
}
