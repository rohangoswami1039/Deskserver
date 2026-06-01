use crate::state::AppState;
use std::sync::{Arc, Mutex};

pub fn render_layout(ui: &mut egui::Ui, state: &Arc<Mutex<AppState>>) {
    ui.heading("Screen Layout");
    ui.label("Drag screens to arrange them.");
    ui.add_space(10.0);

    let available = ui.available_size();
    let canvas_size = egui::vec2(available.x.min(600.0), available.y.min(400.0));

    let (response, painter) =
        ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
    let canvas_rect = response.rect;

    // Draw background
    painter.rect_filled(
        canvas_rect,
        4.0,
        egui::Color32::from_rgb(22, 22, 32),
    );

    // Draw subtle grid lines
    let grid_spacing = 40.0;
    let grid_color = egui::Color32::from_rgba_premultiplied(255, 255, 255, 12);
    let mut x = canvas_rect.min.x + grid_spacing;
    while x < canvas_rect.max.x {
        painter.line_segment(
            [egui::pos2(x, canvas_rect.min.y), egui::pos2(x, canvas_rect.max.y)],
            egui::Stroke::new(1.0, grid_color),
        );
        x += grid_spacing;
    }
    let mut y = canvas_rect.min.y + grid_spacing;
    while y < canvas_rect.max.y {
        painter.line_segment(
            [egui::pos2(canvas_rect.min.x, y), egui::pos2(canvas_rect.max.x, y)],
            egui::Stroke::new(1.0, grid_color),
        );
        y += grid_spacing;
    }

    // Canvas origin offset for screen coordinates
    let origin = canvas_rect.min + egui::vec2(20.0, 20.0);

    // Gather screen rects (scaled 0.5x) before borrowing state mutably
    let screen_rects: Vec<egui::Rect> = {
        let state = state.lock().unwrap();
        state.screens.iter().map(|screen| {
            egui::Rect::from_min_size(
                egui::pos2(origin.x + screen.x, origin.y + screen.y),
                egui::vec2(screen.width * 0.5, screen.height * 0.5),
            )
        }).collect()
    };

    // Handle drag logic
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let mut st = state.lock().unwrap();
            // Find which screen was clicked (search in reverse for top-most)
            let hit = screen_rects.iter().enumerate().rev().find(|(_, r)| r.contains(pos));
            if let Some((idx, rect)) = hit {
                st.dragging_screen = Some(idx);
                st.drag_offset = (pos.x - rect.min.x, pos.y - rect.min.y);
            } else {
                st.dragging_screen = None;
            }
        }
    }

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let mut st = state.lock().unwrap();
            if let Some(idx) = st.dragging_screen {
                let (dx, dy) = st.drag_offset;
                let sw = st.screens[idx].width * 0.5;
                let sh = st.screens[idx].height * 0.5;

                let raw_x = pos.x - origin.x - dx;
                let raw_y = pos.y - origin.y - dy;

                let max_x = canvas_rect.width() - sw - 20.0;
                let max_y = canvas_rect.height() - sh - 20.0;

                let (snapped_x, snapped_y) = crate::edge::snap_screen_position(
                    &st.screens, idx, raw_x, raw_y, 20.0,
                );
                st.screens[idx].x = snapped_x.clamp(0.0, max_x.max(0.0));
                st.screens[idx].y = snapped_y.clamp(0.0, max_y.max(0.0));
            }
        }
    }

    if response.drag_stopped() {
        let mut st = state.lock().unwrap();
        st.dragging_screen = None;
        st.edge_links = crate::edge::compute_edge_links(&st.screens, 20.0);
    }

    // Draw screens
    let dragging_idx = state.lock().unwrap().dragging_screen;
    let screens_snapshot: Vec<(egui::Rect, bool, String, bool, usize)> = {
        let st = state.lock().unwrap();
        st.screens.iter().enumerate().map(|(i, screen)| {
            let rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + screen.x, origin.y + screen.y),
                egui::vec2(screen.width * 0.5, screen.height * 0.5),
            );
            let label = if screen.is_server {
                format!("{}\nServer", screen.name)
            } else {
                format!("{}\nClient", screen.name)
            };
            (rect, screen.is_server, label, i == dragging_idx.unwrap_or(usize::MAX), i)
        }).collect()
    };

    // Render linked edges as green lines
    {
        let st = state.lock().unwrap();
        for link in &st.edge_links {
            let from = &st.screens[link.from_screen];
            let fw = from.width * 0.5;
            let fh = from.height * 0.5;

            let (p1, p2) = match link.from_side {
                crate::state::Side::Right => {
                    let x = origin.x + from.x + fw;
                    let y1 = origin.y + from.y + fh * link.overlap_start;
                    let y2 = origin.y + from.y + fh * link.overlap_end;
                    (egui::Pos2::new(x, y1), egui::Pos2::new(x, y2))
                }
                crate::state::Side::Left => {
                    let x = origin.x + from.x;
                    let y1 = origin.y + from.y + fh * link.overlap_start;
                    let y2 = origin.y + from.y + fh * link.overlap_end;
                    (egui::Pos2::new(x, y1), egui::Pos2::new(x, y2))
                }
                crate::state::Side::Bottom => {
                    let y = origin.y + from.y + fh;
                    let x1 = origin.x + from.x + fw * link.overlap_start;
                    let x2 = origin.x + from.x + fw * link.overlap_end;
                    (egui::Pos2::new(x1, y), egui::Pos2::new(x2, y))
                }
                crate::state::Side::Top => {
                    let y = origin.y + from.y;
                    let x1 = origin.x + from.x + fw * link.overlap_start;
                    let x2 = origin.x + from.x + fw * link.overlap_end;
                    (egui::Pos2::new(x1, y), egui::Pos2::new(x2, y))
                }
            };

            painter.line_segment(
                [p1, p2],
                egui::Stroke::new(3.0, egui::Color32::from_rgb(74, 222, 128)),
            );
        }
    }

    for (rect, is_server, label, is_dragging, _idx) in &screens_snapshot {
        // Fill color
        let fill = if *is_server {
            egui::Color32::from_rgb(30, 58, 95)
        } else {
            egui::Color32::from_rgb(50, 35, 80)
        };
        painter.rect_filled(*rect, 6.0, fill);

        // Border color
        let border_color = if *is_dragging {
            egui::Color32::from_rgb(250, 204, 21) // yellow highlight
        } else if *is_server {
            egui::Color32::from_rgb(125, 211, 252) // #7dd3fc blue
        } else {
            egui::Color32::from_rgb(167, 139, 250) // #a78bfa purple
        };
        let stroke_width = if *is_dragging { 2.5 } else { 1.5 };
        painter.rect_stroke(
            *rect,
            6.0,
            egui::Stroke::new(stroke_width, border_color),
            egui::epaint::StrokeKind::Outside,
        );

        // Role badge
        let role_text = if *is_server { "SERVER" } else { "CLIENT" };
        let role_color = if *is_server {
            egui::Color32::from_rgb(125, 211, 252)
        } else {
            egui::Color32::from_rgb(167, 139, 250)
        };
        painter.text(
            rect.center() + egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            role_text,
            egui::FontId::proportional(10.0),
            role_color,
        );

        // Screen name
        let name = label.lines().next().unwrap_or("");
        painter.text(
            rect.center() - egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );
    }
}
