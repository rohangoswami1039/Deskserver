use super::CaptureEvent;

pub fn run<F: FnMut(CaptureEvent) -> bool + 'static>(_callback: F) {
    todo!("macOS CGEventTap capture — implemented in Task 4")
}
