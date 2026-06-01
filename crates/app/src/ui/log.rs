use crate::state::{AppState, LogLevel};
use std::sync::{Arc, Mutex};

pub fn render_log_panel(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let mut state = state.lock().unwrap();

    ui.horizontal(|ui| {
        ui.strong("Event Log");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Collapse").clicked() {
                state.log_collapsed = true;
            }
            if ui.small_button("Clear").clicked() {
                state.event_log.clear();
            }
        });
    });

    ui.separator();

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for entry in state.event_log.iter() {
                let color = match entry.level {
                    LogLevel::Info => egui::Color32::from_rgb(180, 180, 180),
                    LogLevel::Success => egui::Color32::from_rgb(80, 200, 80),
                    LogLevel::Warning => egui::Color32::from_rgb(255, 200, 80),
                    LogLevel::Mode => egui::Color32::from_rgb(100, 180, 255),
                };
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 120, 120),
                        &entry.timestamp,
                    );
                    ui.colored_label(color, &entry.message);
                });
            }
        });
}

pub fn render_log_collapsed(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let mut state = state.lock().unwrap();
    ui.horizontal(|ui| {
        ui.strong("Event Log");
        if ui.small_button("Expand").clicked() {
            state.log_collapsed = false;
        }
    });
}
