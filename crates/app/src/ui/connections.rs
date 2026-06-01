use crate::state::{AppState, Role};
use std::sync::{Arc, Mutex};

pub fn render_connections(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let mut state = state.lock().unwrap();

    match state.role {
        Role::Server => {
            ui.heading("Connected Clients");
            ui.add_space(8.0);

            if state.connected_clients.is_empty() {
                ui.label("No clients connected.");
            } else {
                for client in &state.connected_clients {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let dot_color = if client.active {
                                egui::Color32::from_rgb(80, 200, 80)
                            } else {
                                egui::Color32::from_rgb(200, 80, 80)
                            };
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(10.0, 10.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().circle_filled(rect.center(), 4.0, dot_color);

                            ui.label(&client.name);
                            ui.label(format!("({})", client.ip));
                            ui.label(format!("{}", client.resolution));
                            ui.label(format!("{:.0}ms", client.latency_ms));
                        });
                    });
                }
            }
        }
        Role::Client => {
            ui.heading("Available Servers");
            ui.add_space(8.0);

            if state.available_servers.is_empty() {
                ui.label("No servers found. Enter an IP address to connect manually.");
            } else {
                for server in &state.available_servers {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(&server.name);
                            ui.label(format!("({})", server.addr));
                            ui.label(format!("{} client(s)", server.client_count));
                            ui.label(format!("{:.0}ms", server.latency_ms));
                            if ui.button("Connect").clicked() {
                                // Connection logic will be added later
                            }
                        });
                    });
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label("Manual Connect");
            ui.horizontal(|ui| {
                ui.label("IP:");
                ui.text_edit_singleline(&mut state.manual_connect_ip);
                if ui.button("Connect").clicked() {
                    // Manual connection logic will be added later
                }
            });
        }
    }
}
