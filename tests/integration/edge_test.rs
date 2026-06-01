use deskserver::edge::*;
use deskserver::state::{ScreenConfig, Side};

fn make_screen(
    name: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rw: u32,
    rh: u32,
    server: bool,
) -> ScreenConfig {
    ScreenConfig {
        name: name.to_string(),
        x,
        y,
        width: w,
        height: h,
        is_server: server,
        real_width: rw,
        real_height: rh,
    }
}

#[test]
fn screens_snapped_right_creates_link() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    assert_eq!(links.len(), 2);
    assert!(links
        .iter()
        .any(|l| l.from_screen == 0 && l.from_side == Side::Right && l.to_screen == 1));
    assert!(links
        .iter()
        .any(|l| l.from_screen == 1 && l.from_side == Side::Left && l.to_screen == 0));
}

#[test]
fn screens_far_apart_no_link() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 300.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    assert_eq!(links.len(), 0);
}

#[test]
fn screens_snapped_bottom_creates_link() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 0.0, 130.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    assert_eq!(links.len(), 2);
    assert!(links
        .iter()
        .any(|l| l.from_side == Side::Bottom && l.to_side == Side::Top));
}

#[test]
fn edge_crossing_right_detected() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    let result = check_edge_crossing(&screens, &links, 0, 1440.0, 450.0);
    assert!(result.is_some());
    let (target, entry_x, entry_y) = result.unwrap();
    assert_eq!(target, 1);
    assert!(entry_x < 10.0); // Near left edge
    assert!((entry_y - 540.0).abs() < 10.0); // 50% of 1080
}

#[test]
fn edge_crossing_not_at_edge() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    let result = check_edge_crossing(&screens, &links, 0, 720.0, 450.0);
    assert!(result.is_none());
}

#[test]
fn virtual_cursor_exit_left() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    // Virtual cursor went past left edge of Windows screen
    let result = check_virtual_cursor_exit(&screens, &links, 1, -10.0, 500.0);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 0); // Return to Mac
}
