use crate::state::AppState;
use std::sync::{Arc, Mutex};

pub fn render_layout(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    let state = state.lock().unwrap();

    ui.heading("Screen Layout");
    ui.label("Arrange screens by dragging them (coming soon).");
    ui.add_space(10.0);

    let available = ui.available_size();
    let canvas_size = egui::vec2(available.x.min(600.0), available.y.min(400.0));

    let (response, painter) =
        ui.allocate_painter(canvas_size, egui::Sense::hover());
    let canvas_rect = response.rect;

    // Draw background
    painter.rect_filled(
        canvas_rect,
        4.0,
        egui::Color32::from_rgb(30, 30, 40),
    );

    // Draw each screen
    let offset = canvas_rect.min + egui::vec2(50.0, 50.0);
    for screen in &state.screens {
        let screen_rect = egui::Rect::from_min_size(
            egui::pos2(offset.x + screen.x, offset.y + screen.y),
            egui::vec2(screen.width * 0.5, screen.height * 0.5),
        );

        let fill = if screen.is_server {
            egui::Color32::from_rgb(50, 80, 120)
        } else {
            egui::Color32::from_rgb(60, 60, 80)
        };

        painter.rect_filled(screen_rect, 4.0, fill);
        painter.rect_stroke(
            screen_rect,
            4.0,
            egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 140, 180)),
            egui::StrokeKind::Outside,
        );

        // Screen label
        let label = if screen.is_server {
            format!("{} (Server)", screen.name)
        } else {
            screen.name.clone()
        };
        painter.text(
            screen_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );
    }
}
