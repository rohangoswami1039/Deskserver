use super::CaptureEvent;
use deskserver_common::keymap::macos_flags_to_modifiers;
use deskserver_common::MouseButton;
use std::cell::RefCell;
use std::ffi::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {}

// Raw CoreGraphics FFI — we use this instead of the core-graphics crate's
// CGEventTap::new because the crate may not correctly build the event mask
// for keyboard events.

type CGEventRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CGEventMask = u64;
type CGEventType_ = u32;
type CGEventField = u32;
type CGDirectDisplayID = u32;

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: CGEventType_,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

// Event types
const KCG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const KCG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const KCG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const KCG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const KCG_EVENT_MOUSE_MOVED: u32 = 5;
const KCG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const KCG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
const KCG_EVENT_FLAGS_CHANGED: u32 = 12;
const KCG_EVENT_SCROLL_WHEEL: u32 = 22;
const KCG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const KCG_EVENT_OTHER_MOUSE_UP: u32 = 26;
const KCG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;
const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;

// Event fields
const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
const KCG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11;
const KCG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 12;

// Tap location/placement/options
const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const KCG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

// Modifier flag masks
const FLAG_SHIFT: u64 = 0x020000;
const FLAG_CONTROL: u64 = 0x040000;
const FLAG_OPTION: u64 = 0x080000;
const FLAG_COMMAND: u64 = 0x100000;
const FLAG_CAPS_LOCK: u64 = 0x010000;

extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;

    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: i64,
    ) -> CFRunLoopSourceRef;

    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();

    static kCFRunLoopCommonModes: CFStringRef;

    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;

    fn CGMainDisplayID() -> CGDirectDisplayID;
    fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> usize;
    fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> usize;
    fn CGDisplayHideCursor(display: CGDirectDisplayID) -> i32;
    fn CGDisplayShowCursor(display: CGDirectDisplayID) -> i32;
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
}

thread_local! {
    static CALLBACK: RefCell<Option<Box<dyn FnMut(CaptureEvent) -> bool>>> = RefCell::new(None);
    static TAP_PORT: RefCell<CFMachPortRef> = RefCell::new(std::ptr::null_mut());
}

fn flags_changed_is_press(keycode: u32, flags_raw: u64) -> bool {
    match keycode {
        56 | 60 => flags_raw & FLAG_SHIFT != 0,
        59 | 62 => flags_raw & FLAG_CONTROL != 0,
        58 | 61 => flags_raw & FLAG_OPTION != 0,
        55 | 54 => flags_raw & FLAG_COMMAND != 0,
        57 => flags_raw & FLAG_CAPS_LOCK != 0,
        _ => true,
    }
}

unsafe extern "C" fn tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType_,
    event: CGEventRef,
    _user_info: *mut c_void,
) -> CGEventRef {
    // Re-enable tap on timeout
    if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        println!("[CAPTURE] Tap disabled by timeout — re-enabling");
        TAP_PORT.with(|p| {
            let port = *p.borrow();
            if !port.is_null() {
                CGEventTapEnable(port, true);
            }
        });
        return event;
    }

    let capture_event = match event_type {
        KCG_EVENT_MOUSE_MOVED | KCG_EVENT_LEFT_MOUSE_DRAGGED
        | KCG_EVENT_RIGHT_MOUSE_DRAGGED | KCG_EVENT_OTHER_MOUSE_DRAGGED => {
            let loc = CGEventGetLocation(event);
            Some(CaptureEvent::MouseMove { x: loc.x, y: loc.y })
        }
        KCG_EVENT_LEFT_MOUSE_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: true }),
        KCG_EVENT_LEFT_MOUSE_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Left, pressed: false }),
        KCG_EVENT_RIGHT_MOUSE_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: true }),
        KCG_EVENT_RIGHT_MOUSE_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Right, pressed: false }),
        KCG_EVENT_OTHER_MOUSE_DOWN => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: true }),
        KCG_EVENT_OTHER_MOUSE_UP => Some(CaptureEvent::MouseButton { button: MouseButton::Middle, pressed: false }),
        KCG_EVENT_SCROLL_WHEEL => {
            let dy = CGEventGetIntegerValueField(event, KCG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
            let dx = CGEventGetIntegerValueField(event, KCG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
            Some(CaptureEvent::Wheel { dx, dy })
        }
        KCG_EVENT_KEY_DOWN => {
            let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u32;
            let flags = CGEventGetFlags(event);
            let modifiers = macos_flags_to_modifiers(flags);
            Some(CaptureEvent::KeyDown { keycode, modifiers })
        }
        KCG_EVENT_KEY_UP => {
            let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u32;
            let flags = CGEventGetFlags(event);
            let modifiers = macos_flags_to_modifiers(flags);
            Some(CaptureEvent::KeyUp { keycode, modifiers })
        }
        KCG_EVENT_FLAGS_CHANGED => {
            let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u32;
            let flags = CGEventGetFlags(event);
            let modifiers = macos_flags_to_modifiers(flags);
            let pressed = flags_changed_is_press(keycode, flags);
            if pressed {
                Some(CaptureEvent::KeyDown { keycode, modifiers })
            } else {
                Some(CaptureEvent::KeyUp { keycode, modifiers })
            }
        }
        _ => None,
    };

    if let Some(ce) = capture_event {
        let suppress = CALLBACK.with(|cb| {
            if let Some(ref mut f) = *cb.borrow_mut() {
                f(ce)
            } else {
                false
            }
        });
        if suppress {
            return std::ptr::null_mut(); // Suppress the event
        }
    }

    event // Pass through
}

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(callback));
    });

    // Build event mask with all event types we care about
    let mask: CGEventMask =
        (1 << KCG_EVENT_LEFT_MOUSE_DOWN)
        | (1 << KCG_EVENT_LEFT_MOUSE_UP)
        | (1 << KCG_EVENT_RIGHT_MOUSE_DOWN)
        | (1 << KCG_EVENT_RIGHT_MOUSE_UP)
        | (1 << KCG_EVENT_MOUSE_MOVED)
        | (1 << KCG_EVENT_LEFT_MOUSE_DRAGGED)
        | (1 << KCG_EVENT_RIGHT_MOUSE_DRAGGED)
        | (1 << KCG_EVENT_KEY_DOWN)
        | (1 << KCG_EVENT_KEY_UP)
        | (1 << KCG_EVENT_FLAGS_CHANGED)
        | (1 << KCG_EVENT_SCROLL_WHEEL)
        | (1 << KCG_EVENT_OTHER_MOUSE_DOWN)
        | (1 << KCG_EVENT_OTHER_MOUSE_UP)
        | (1 << KCG_EVENT_OTHER_MOUSE_DRAGGED);

    unsafe {
        let tap = CGEventTapCreate(
            KCG_HID_EVENT_TAP,
            KCG_HEAD_INSERT_EVENT_TAP,
            KCG_EVENT_TAP_OPTION_DEFAULT,
            mask,
            tap_callback,
            std::ptr::null_mut(),
        );

        if tap.is_null() {
            eprintln!("[CAPTURE] ERROR: Failed to create CGEventTap.");
            eprintln!("[CAPTURE] macOS Accessibility permission is required.");
            eprintln!("[CAPTURE] Go to: System Settings > Privacy & Security > Accessibility");
            std::process::exit(1);
        }

        TAP_PORT.with(|p| *p.borrow_mut() = tap);

        CGEventTapEnable(tap, true);
        println!("[CAPTURE] macOS CGEventTap active (raw FFI). Keyboard + mouse capture enabled.");

        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if source.is_null() {
            eprintln!("[CAPTURE] ERROR: Failed to create run loop source.");
            std::process::exit(1);
        }

        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);

        CFRunLoopRun(); // Block forever
    }
}

// Public helpers for cursor control (used by main.rs toggle_mode)
pub fn hide_cursor() {
    unsafe { CGDisplayHideCursor(CGMainDisplayID()); }
}

pub fn show_cursor() {
    unsafe { CGDisplayShowCursor(CGMainDisplayID()); }
}

pub fn warp_cursor_to_center() {
    unsafe {
        let display = CGMainDisplayID();
        let w = CGDisplayPixelsWide(display) as f64;
        let h = CGDisplayPixelsHigh(display) as f64;
        CGWarpMouseCursorPosition(CGPoint { x: w / 2.0, y: h / 2.0 });
    }
}
