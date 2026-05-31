use super::CaptureEvent;

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(_callback: F) {
    todo!("Windows SetWindowsHookEx capture — implemented in Task 5")
}
