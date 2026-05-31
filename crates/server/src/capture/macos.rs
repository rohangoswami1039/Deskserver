use super::CaptureEvent;
use core_foundation::mach_port::CFMachPort;
use core_foundation::runloop::{CFRunLoop, CFRunLoopSource};
use core_foundation::runloop::kCFRunLoopCommonModes;
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, EventField,
};
use deskserver_common::keymap::macos_flags_to_modifiers;
use deskserver_common::MouseButton;
use std::cell::RefCell;

extern "C" {
    fn CGEventTapEnable(tap: core_foundation::mach_port::CFMachPortRef, enable: bool);
}

/// Store the mach port so we can re-enable the tap on timeout.
thread_local! {
    static TAP_MACH_PORT: RefCell<Option<CFMachPort>> = RefCell::new(None);
}

/// Modifier flag masks for detecting individual modifier key press/release
/// from FlagsChanged events.
const FLAG_SHIFT: u64 = 0x020000;
const FLAG_CONTROL: u64 = 0x040000;
const FLAG_OPTION: u64 = 0x080000;
const FLAG_COMMAND: u64 = 0x100000;
const FLAG_CAPS_LOCK: u64 = 0x010000;

/// Determine if a FlagsChanged event is a key-down or key-up by checking
/// whether the modifier flag corresponding to the keycode is currently set.
fn flags_changed_is_press(keycode: u32, flags_raw: u64) -> bool {
    match keycode {
        // Shift keys (left=56, right=60)
        56 | 60 => flags_raw & FLAG_SHIFT != 0,
        // Control keys (left=59, right=62)
        59 | 62 => flags_raw & FLAG_CONTROL != 0,
        // Option/Alt keys (left=58, right=61)
        58 | 61 => flags_raw & FLAG_OPTION != 0,
        // Command keys (left=55, right=54)
        55 | 54 => flags_raw & FLAG_COMMAND != 0,
        // Caps Lock (57)
        57 => flags_raw & FLAG_CAPS_LOCK != 0,
        // Unknown modifier key — treat as press
        _ => true,
    }
}

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    let callback = RefCell::new(callback);

    let events_of_interest = vec![
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGEventType::RightMouseDown,
        CGEventType::RightMouseUp,
        CGEventType::OtherMouseDown,
        CGEventType::OtherMouseUp,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDragged,
        CGEventType::ScrollWheel,
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::FlagsChanged,
    ];

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        events_of_interest,
        move |_proxy: CGEventTapProxy, etype: CGEventType, event: &CGEvent| -> Option<CGEvent> {
            // Handle tap disabled by timeout — re-enable the tap.
            if matches!(etype, CGEventType::TapDisabledByTimeout) {
                TAP_MACH_PORT.with(|port| {
                    if let Some(ref tap) = *port.borrow() {
                        unsafe {
                            CGEventTapEnable(
                                core_foundation::base::TCFType::as_concrete_TypeRef(tap),
                                true,
                            );
                        }
                    }
                });
                return None;
            }

            let capture_event = match etype {
                CGEventType::MouseMoved
                | CGEventType::LeftMouseDragged
                | CGEventType::RightMouseDragged
                | CGEventType::OtherMouseDragged => {
                    let loc = event.location();
                    CaptureEvent::MouseMove { x: loc.x, y: loc.y }
                }

                CGEventType::LeftMouseDown => CaptureEvent::MouseButton {
                    button: MouseButton::Left,
                    pressed: true,
                },
                CGEventType::LeftMouseUp => CaptureEvent::MouseButton {
                    button: MouseButton::Left,
                    pressed: false,
                },
                CGEventType::RightMouseDown => CaptureEvent::MouseButton {
                    button: MouseButton::Right,
                    pressed: true,
                },
                CGEventType::RightMouseUp => CaptureEvent::MouseButton {
                    button: MouseButton::Right,
                    pressed: false,
                },
                CGEventType::OtherMouseDown => CaptureEvent::MouseButton {
                    button: MouseButton::Middle,
                    pressed: true,
                },
                CGEventType::OtherMouseUp => CaptureEvent::MouseButton {
                    button: MouseButton::Middle,
                    pressed: false,
                },

                CGEventType::ScrollWheel => {
                    let dy =
                        event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
                    let dx =
                        event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
                    CaptureEvent::Wheel { dx, dy }
                }

                CGEventType::KeyDown => {
                    let keycode = event
                        .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        as u32;
                    let flags_raw = event.get_flags().bits();
                    let modifiers = macos_flags_to_modifiers(flags_raw);
                    CaptureEvent::KeyDown { keycode, modifiers }
                }
                CGEventType::KeyUp => {
                    let keycode = event
                        .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        as u32;
                    let flags_raw = event.get_flags().bits();
                    let modifiers = macos_flags_to_modifiers(flags_raw);
                    CaptureEvent::KeyUp { keycode, modifiers }
                }

                CGEventType::FlagsChanged => {
                    let keycode = event
                        .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        as u32;
                    let flags_raw = event.get_flags().bits();
                    let modifiers = macos_flags_to_modifiers(flags_raw);
                    let pressed = flags_changed_is_press(keycode, flags_raw);
                    if pressed {
                        CaptureEvent::KeyDown { keycode, modifiers }
                    } else {
                        CaptureEvent::KeyUp { keycode, modifiers }
                    }
                }

                // Ignore other event types
                _ => return Some(event.clone()),
            };

            let suppress = callback.borrow_mut()(capture_event);
            if suppress {
                None
            } else {
                Some(event.clone())
            }
        },
    );

    let tap = match tap {
        Ok(tap) => tap,
        Err(()) => {
            eprintln!("ERROR: Failed to create CGEventTap.");
            eprintln!("       macOS Accessibility permission is required.");
            eprintln!("       Go to System Settings > Privacy & Security > Accessibility");
            eprintln!("       and grant permission to this application.");
            std::process::exit(1);
        }
    };

    // Store the mach port for re-enabling on timeout.
    TAP_MACH_PORT.with(|port| {
        *port.borrow_mut() = Some(tap.mach_port.clone());
    });

    let run_loop_source: CFRunLoopSource = tap
        .mach_port
        .create_runloop_source(0)
        .expect("Failed to create CFRunLoopSource from event tap mach port");

    let run_loop = CFRunLoop::get_current();
    unsafe {
        run_loop.add_source(&run_loop_source, kCFRunLoopCommonModes);
    }

    tap.enable();

    // Block forever — runs the macOS event loop on the current thread.
    CFRunLoop::run_current();
}
