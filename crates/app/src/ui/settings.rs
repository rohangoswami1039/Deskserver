use crate::state::{AppState, Role};
use std::sync::{Arc, Mutex};

pub fn render_settings(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let mut state = state.lock().unwrap();

    ui.heading("Settings");
    ui.add_space(12.0);

    egui::Grid::new("settings_grid")
        .num_columns(2)
        .spacing([20.0, 8.0])
        .show(ui, |ui| {
            ui.label("Machine Name:");
            ui.text_edit_singleline(&mut state.machine_name);
            ui.end_row();

            ui.label("Role:");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.role, Role::Server, "Server");
                ui.selectable_value(&mut state.role, Role::Client, "Client");
            });
            ui.end_row();

            ui.label("Port:");
            let mut port_str = state.port.to_string();
            if ui.text_edit_singleline(&mut port_str).changed() {
                if let Ok(p) = port_str.parse::<u16>() {
                    state.port = p;
                }
            }
            ui.end_row();

            ui.label("Hotkey:");
            ui.label("Scroll Lock (default)");
            ui.end_row();

            ui.label("Auto Start:");
            ui.checkbox(&mut state.auto_start, "Start on login");
            ui.end_row();

            ui.label("Auto Connect:");
            ui.checkbox(&mut state.auto_connect, "Connect automatically");
            ui.end_row();
        });
}
