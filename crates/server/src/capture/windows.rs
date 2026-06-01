use super::CaptureEvent;
use deskserver_common::keymap::windows_mods_to_modifiers;
use deskserver_common::MouseButton;
use std::cell::RefCell;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
    TranslateMessage, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

thread_local! {
    static MOUSE_HOOK: RefCell<HHOOK> = RefCell::new(HHOOK::default());
    static KB_HOOK: RefCell<HHOOK> = RefCell::new(HHOOK::default());
    static CALLBACK: RefCell<Option<Box<dyn FnMut(CaptureEvent) -> bool>>> = RefCell::new(None);
}

fn get_modifiers() -> u8 {
    unsafe {
        let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) < 0;
        let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) < 0;
        let alt = GetAsyncKeyState(VK_MENU.0 as i32) < 0;
        let win = GetAsyncKeyState(VK_LWIN.0 as i32) < 0;
        windows_mods_to_modifiers(ctrl, shift, alt, win)
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let event = match wparam.0 as u32 {
            WM_MOUSEMOVE => Some(CaptureEvent::MouseMove {
                x: info.pt.x as f64,
                y: info.pt.y as f64,
                delta_x: 0.0, // Windows deltas handled differently
                delta_y: 0.0,
            }),
            WM_LBUTTONDOWN => Some(CaptureEvent::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            }),
            WM_LBUTTONUP => Some(CaptureEvent::MouseButton {
                button: MouseButton::Left,
                pressed: false,
            }),
            WM_RBUTTONDOWN => Some(CaptureEvent::MouseButton {
                button: MouseButton::Right,
                pressed: true,
            }),
            WM_RBUTTONUP => Some(CaptureEvent::MouseButton {
                button: MouseButton::Right,
                pressed: false,
            }),
            WM_MBUTTONDOWN => Some(CaptureEvent::MouseButton {
                button: MouseButton::Middle,
                pressed: true,
            }),
            WM_MBUTTONUP => Some(CaptureEvent::MouseButton {
                button: MouseButton::Middle,
                pressed: false,
            }),
            WM_MOUSEWHEEL => {
                let delta = (info.mouseData >> 16) as i16;
                Some(CaptureEvent::Wheel {
                    dx: 0,
                    dy: delta as i64 / 120,
                })
            }
            _ => None,
        };

        if let Some(ce) = event {
            let suppress = CALLBACK.with(|cb| {
                if let Some(ref mut f) = *cb.borrow_mut() {
                    f(ce)
                } else {
                    false
                }
            });
            if suppress {
                return LRESULT(1);
            }
        }
    }
    MOUSE_HOOK.with(|h| unsafe { CallNextHookEx(*h.borrow(), code, wparam, lparam) })
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let mods = get_modifiers();
        let event = match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(CaptureEvent::KeyDown {
                keycode: info.vkCode,
                modifiers: mods,
            }),
            WM_KEYUP | WM_SYSKEYUP => Some(CaptureEvent::KeyUp {
                keycode: info.vkCode,
                modifiers: mods,
            }),
            _ => None,
        };

        if let Some(ce) = event {
            let suppress = CALLBACK.with(|cb| {
                if let Some(ref mut f) = *cb.borrow_mut() {
                    f(ce)
                } else {
                    false
                }
            });
            if suppress {
                return LRESULT(1);
            }
        }
    }
    KB_HOOK.with(|h| unsafe { CallNextHookEx(*h.borrow(), code, wparam, lparam) })
}

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(callback: F) {
    CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(callback));
    });

    unsafe {
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
            .expect("[CAPTURE] Failed to install mouse hook");
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
            .expect("[CAPTURE] Failed to install keyboard hook");

        MOUSE_HOOK.with(|h| *h.borrow_mut() = mouse);
        KB_HOOK.with(|h| *h.borrow_mut() = kb);

        println!("[CAPTURE] Windows hooks installed (mouse + keyboard).");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
