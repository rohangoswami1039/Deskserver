mod discovery;
mod network;
mod state;
mod tray;
mod ui;

use state::{AppState, LogLevel};
use std::sync::{Arc, Mutex};
use ui::DeskserverApp;

fn main() -> eframe::Result {
    let state = Arc::new(Mutex::new(AppState::default()));

    let _tray = tray::create_tray();

    let net_tx = network::spawn_network_thread(state.clone());
    state.lock().unwrap().network_tx = Some(net_tx);

    {
        let mut s = state.lock().unwrap();
        s.log("Deskserver started", LogLevel::Info);
        let name = s.machine_name.clone();
        s.log(format!("Machine name: {}", name), LogLevel::Info);
        s.log("Role: Server", LogLevel::Info);
        s.log("Listening on port 24800", LogLevel::Info);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 500.0])
            .with_min_inner_size([500.0, 350.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Deskserver",
        options,
        Box::new(move |cc| {
            // Apply custom dark theme matching mockup design (deep blue/purple tones)
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(18, 18, 32);
            visuals.window_fill = egui::Color32::from_rgb(22, 22, 40);
            visuals.extreme_bg_color = egui::Color32::from_rgb(10, 10, 20);
            visuals.faint_bg_color = egui::Color32::from_rgb(25, 25, 45);
            cc.egui_ctx.set_visuals(visuals);

            Ok(Box::new(DeskserverApp::new(state.clone())))
        }),
    )
}
