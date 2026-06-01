pub mod connections;
pub mod layout;
pub mod log;
pub mod settings;
pub mod status;

use crate::state::{AppState, Tab};
use std::sync::{Arc, Mutex};

pub struct DeskserverApp {
    pub state: Arc<Mutex<AppState>>,
}

impl DeskserverApp {
    pub fn new(state: Arc<Mutex<AppState>>) -> Self {
        Self { state }
    }
}

impl eframe::App for DeskserverApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint every 100ms for live updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        // Top status bar
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            status::render_status_bar(ui, &self.state);
        });

        // Bottom log panel (collapsible)
        {
            let collapsed = {
                let state = self.state.lock().unwrap();
                state.log_collapsed
            };
            if !collapsed {
                egui::TopBottomPanel::bottom("log_panel")
                    .resizable(true)
                    .min_height(80.0)
                    .default_height(150.0)
                    .show(ctx, |ui| {
                        log::render_log_panel(ui, &self.state);
                    });
            } else {
                egui::TopBottomPanel::bottom("log_panel_collapsed")
                    .show(ctx, |ui| {
                        log::render_log_collapsed(ui, &self.state);
                    });
            }
        }

        // Central panel with tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut state = self.state.lock().unwrap();
                ui.selectable_value(&mut state.active_tab, Tab::ScreenLayout, "Screen Layout");
                ui.selectable_value(&mut state.active_tab, Tab::Connections, "Connections");
                ui.selectable_value(&mut state.active_tab, Tab::Settings, "Settings");
            });

            ui.separator();

            let active_tab = {
                let state = self.state.lock().unwrap();
                state.active_tab.clone()
            };

            match active_tab {
                Tab::ScreenLayout => layout::render_layout(ui, &self.state),
                Tab::Connections => connections::render_connections(ui, &self.state),
                Tab::Settings => settings::render_settings(ui, &self.state),
            }
        });
    }
}
