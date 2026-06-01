mod state;
mod ui;

use state::{AppState, LogLevel};
use std::sync::{Arc, Mutex};
use ui::DeskserverApp;

fn main() -> eframe::Result {
    let state = Arc::new(Mutex::new(AppState::default()));

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
            // Apply dark visuals with custom accent colors
            let mut visuals = egui::Visuals::dark();
            visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 220));
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(25, 25, 30);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 50);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 70);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 70, 90);
            visuals.selection.bg_fill = egui::Color32::from_rgb(50, 80, 130);
            visuals.panel_fill = egui::Color32::from_rgb(20, 20, 25);
            visuals.window_fill = egui::Color32::from_rgb(20, 20, 25);
            cc.egui_ctx.set_visuals(visuals);

            Ok(Box::new(DeskserverApp::new(state.clone())))
        }),
    )
}
