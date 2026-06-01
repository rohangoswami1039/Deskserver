use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};

pub struct TrayState {
    /// Kept alive for the lifetime of the app — dropping it removes the tray icon.
    _tray: tray_icon::TrayIcon,
    pub toggle_item: MenuItem,
    pub open_item: MenuItem,
    pub quit_item: MenuItem,
}

/// Build a 16×16 solid-colour RGBA icon so we never depend on an image file.
fn make_icon() -> Icon {
    let size: u32 = 16;
    let rgba: Vec<u8> = (0..size * size)
        .flat_map(|_| [0x4A_u8, 0x90, 0xD9, 0xFF])
        .collect();
    Icon::from_rgba(rgba, size, size).expect("valid icon data")
}

pub fn create_tray() -> TrayState {
    let toggle_item = MenuItem::new("Toggle Mode", true, None);
    let open_item = MenuItem::new("Open Deskserver", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::with_items(&[
        &toggle_item,
        &PredefinedMenuItem::separator(),
        &open_item,
        &quit_item,
    ])
    .expect("failed to build tray menu");

    let icon = make_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Deskserver")
        .with_icon(icon)
        .build()
        .expect("failed to create tray icon");

    println!("[TRAY] System tray icon created");

    TrayState {
        _tray: tray,
        toggle_item,
        open_item,
        quit_item,
    }
}

/// Poll for pending tray menu events.  Returns `true` if the user clicked Quit.
pub fn poll_tray_events(tray: &TrayState) -> bool {
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id() == tray.quit_item.id() {
            println!("[TRAY] Quit clicked");
            return true;
        } else if event.id() == tray.toggle_item.id() {
            println!("[TRAY] Toggle Mode clicked");
        } else if event.id() == tray.open_item.id() {
            println!("[TRAY] Open Deskserver clicked");
        }
    }
    false
}
