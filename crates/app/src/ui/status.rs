use crate::state::{AppState, InputMode, Role};
use std::sync::{Arc, Mutex};

pub fn render_status_bar(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let state = state.lock().unwrap();

    ui.horizontal(|ui| {
        ui.heading("Deskserver");
        ui.separator();

        // Connection status dot
        let connected = match state.role {
            Role::Server => !state.connected_clients.is_empty(),
            Role::Client => state.connected_server.is_some(),
        };
        let color = if connected {
            egui::Color32::from_rgb(80, 200, 80)
        } else {
            egui::Color32::from_rgb(200, 80, 80)
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 5.0, color);

        let status_text = if connected { "Connected" } else { "Disconnected" };
        ui.label(status_text);

        ui.separator();

        // Mode
        let mode_text = match state.mode {
            InputMode::Local => "LOCAL",
            InputMode::Remote => "REMOTE",
        };
        let mode_color = match state.mode {
            InputMode::Local => egui::Color32::from_rgb(100, 180, 255),
            InputMode::Remote => egui::Color32::from_rgb(255, 180, 100),
        };
        ui.colored_label(mode_color, mode_text);

        ui.separator();

        // Role
        let role_text = match state.role {
            Role::Server => "Server",
            Role::Client => "Client",
        };
        ui.label(role_text);
    });
}
