use crate::state::{EdgeLink, ScreenConfig, Side};

/// Compute the vertical overlap between two screens in normalized [0,1] range
/// relative to screen A's height. Returns (start, end) or (0,0) if no overlap.
fn vertical_overlap(a: &ScreenConfig, b: &ScreenConfig) -> (f32, f32) {
    let a_top = a.y;
    let a_bottom = a.y + a.height;
    let b_top = b.y;
    let b_bottom = b.y + b.height;

    let overlap_top = a_top.max(b_top);
    let overlap_bottom = a_bottom.min(b_bottom);

    if overlap_top >= overlap_bottom {
        return (0.0, 0.0);
    }

    let start = (overlap_top - a_top) / a.height;
    let end = (overlap_bottom - a_top) / a.height;
    (start, end)
}

/// Compute the horizontal overlap between two screens in normalized [0,1] range
/// relative to screen A's width. Returns (start, end) or (0,0) if no overlap.
fn horizontal_overlap(a: &ScreenConfig, b: &ScreenConfig) -> (f32, f32) {
    let a_left = a.x;
    let a_right = a.x + a.width;
    let b_left = b.x;
    let b_right = b.x + b.width;

    let overlap_left = a_left.max(b_left);
    let overlap_right = a_right.min(b_right);

    if overlap_left >= overlap_right {
        return (0.0, 0.0);
    }

    let start = (overlap_left - a_left) / a.width;
    let end = (overlap_right - a_left) / a.width;
    (start, end)
}

/// Iterates all pairs of screens and checks if any edges are within snap_threshold.
/// Creates bidirectional EdgeLink entries for adjacent edges with overlap.
pub fn compute_edge_links(screens: &[ScreenConfig], snap_threshold: f32) -> Vec<EdgeLink> {
    let mut links = Vec::new();

    for i in 0..screens.len() {
        for j in (i + 1)..screens.len() {
            let a = &screens[i];
            let b = &screens[j];

            // Check right edge of A vs left edge of B
            let a_right = a.x + a.width;
            let b_left = b.x;
            if (a_right - b_left).abs() <= snap_threshold {
                let (os, oe) = vertical_overlap(a, b);
                if os < oe {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Right,
                        to_screen: j,
                        to_side: Side::Left,
                        overlap_start: os,
                        overlap_end: oe,
                    });
                    // Compute overlap relative to B
                    let (bs, be) = vertical_overlap(b, a);
                    links.push(EdgeLink {
                        from_screen: j,
                        from_side: Side::Left,
                        to_screen: i,
                        to_side: Side::Right,
                        overlap_start: bs,
                        overlap_end: be,
                    });
                }
            }

            // Check left edge of A vs right edge of B
            let a_left = a.x;
            let b_right = b.x + b.width;
            if (a_left - b_right).abs() <= snap_threshold {
                let (os, oe) = vertical_overlap(a, b);
                if os < oe {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Left,
                        to_screen: j,
                        to_side: Side::Right,
                        overlap_start: os,
                        overlap_end: oe,
                    });
                    let (bs, be) = vertical_overlap(b, a);
                    links.push(EdgeLink {
                        from_screen: j,
                        from_side: Side::Right,
                        to_screen: i,
                        to_side: Side::Left,
                        overlap_start: bs,
                        overlap_end: be,
                    });
                }
            }

            // Check bottom edge of A vs top edge of B
            let a_bottom = a.y + a.height;
            let b_top = b.y;
            if (a_bottom - b_top).abs() <= snap_threshold {
                let (os, oe) = horizontal_overlap(a, b);
                if os < oe {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Bottom,
                        to_screen: j,
                        to_side: Side::Top,
                        overlap_start: os,
                        overlap_end: oe,
                    });
                    let (bs, be) = horizontal_overlap(b, a);
                    links.push(EdgeLink {
                        from_screen: j,
                        from_side: Side::Top,
                        to_screen: i,
                        to_side: Side::Bottom,
                        overlap_start: bs,
                        overlap_end: be,
                    });
                }
            }

            // Check top edge of A vs bottom edge of B
            let a_top = a.y;
            let b_bottom = b.y + b.height;
            if (a_top - b_bottom).abs() <= snap_threshold {
                let (os, oe) = horizontal_overlap(a, b);
                if os < oe {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Top,
                        to_screen: j,
                        to_side: Side::Bottom,
                        overlap_start: os,
                        overlap_end: oe,
                    });
                    let (bs, be) = horizontal_overlap(b, a);
                    links.push(EdgeLink {
                        from_screen: j,
                        from_side: Side::Bottom,
                        to_screen: i,
                        to_side: Side::Top,
                        overlap_start: bs,
                        overlap_end: be,
                    });
                }
            }
        }
    }

    links
}

/// Checks if cursor at (cursor_x, cursor_y) on screen `current_screen` is hitting
/// any linked edge. Uses real screen coordinates.
/// Edge threshold: 5.0 pixels from the edge.
/// Returns (target_screen_index, entry_x, entry_y) or None.
pub fn check_edge_crossing(
    screens: &[ScreenConfig],
    links: &[EdgeLink],
    current_screen: usize,
    cursor_x: f64,
    cursor_y: f64,
) -> Option<(usize, f64, f64)> {
    let screen = &screens[current_screen];
    let rw = screen.real_width as f64;
    let rh = screen.real_height as f64;
    let threshold = 5.0;

    for link in links {
        if link.from_screen != current_screen {
            continue;
        }

        let at_edge = match link.from_side {
            Side::Right => cursor_x >= rw - threshold,
            Side::Left => cursor_x <= threshold,
            Side::Bottom => cursor_y >= rh - threshold,
            Side::Top => cursor_y <= threshold,
        };

        if !at_edge {
            continue;
        }

        // Check if cursor is within the overlap range (in real coordinates)
        let in_overlap = match link.from_side {
            Side::Right | Side::Left => {
                let norm_y = cursor_y / rh;
                norm_y >= link.overlap_start as f64 && norm_y <= link.overlap_end as f64
            }
            Side::Bottom | Side::Top => {
                let norm_x = cursor_x / rw;
                norm_x >= link.overlap_start as f64 && norm_x <= link.overlap_end as f64
            }
        };

        if !in_overlap {
            continue;
        }

        let target = &screens[link.to_screen];
        let trw = target.real_width as f64;
        let trh = target.real_height as f64;

        // Compute proportional entry position on target screen
        let (entry_x, entry_y) = match link.from_side {
            Side::Right => {
                // Enter from left of target
                let norm_y = (cursor_y / rh - link.overlap_start as f64)
                    / (link.overlap_end - link.overlap_start) as f64;
                // Find the corresponding overlap on the target side
                let target_link = links.iter().find(|l| {
                    l.from_screen == link.to_screen
                        && l.to_screen == current_screen
                        && l.from_side == link.to_side
                });
                if let Some(tl) = target_link {
                    let target_y = (tl.overlap_start as f64
                        + norm_y * (tl.overlap_end - tl.overlap_start) as f64)
                        * trh;
                    (0.0, target_y)
                } else {
                    (0.0, norm_y * trh)
                }
            }
            Side::Left => {
                // Enter from right of target
                let norm_y = (cursor_y / rh - link.overlap_start as f64)
                    / (link.overlap_end - link.overlap_start) as f64;
                let target_link = links.iter().find(|l| {
                    l.from_screen == link.to_screen
                        && l.to_screen == current_screen
                        && l.from_side == link.to_side
                });
                if let Some(tl) = target_link {
                    let target_y = (tl.overlap_start as f64
                        + norm_y * (tl.overlap_end - tl.overlap_start) as f64)
                        * trh;
                    (trw - 1.0, target_y)
                } else {
                    (trw - 1.0, norm_y * trh)
                }
            }
            Side::Bottom => {
                // Enter from top of target
                let norm_x = (cursor_x / rw - link.overlap_start as f64)
                    / (link.overlap_end - link.overlap_start) as f64;
                let target_link = links.iter().find(|l| {
                    l.from_screen == link.to_screen
                        && l.to_screen == current_screen
                        && l.from_side == link.to_side
                });
                if let Some(tl) = target_link {
                    let target_x = (tl.overlap_start as f64
                        + norm_x * (tl.overlap_end - tl.overlap_start) as f64)
                        * trw;
                    (target_x, 0.0)
                } else {
                    (norm_x * trw, 0.0)
                }
            }
            Side::Top => {
                // Enter from bottom of target
                let norm_x = (cursor_x / rw - link.overlap_start as f64)
                    / (link.overlap_end - link.overlap_start) as f64;
                let target_link = links.iter().find(|l| {
                    l.from_screen == link.to_screen
                        && l.to_screen == current_screen
                        && l.from_side == link.to_side
                });
                if let Some(tl) = target_link {
                    let target_x = (tl.overlap_start as f64
                        + norm_x * (tl.overlap_end - tl.overlap_start) as f64)
                        * trw;
                    (target_x, trh - 1.0)
                } else {
                    (norm_x * trw, trh - 1.0)
                }
            }
        };

        return Some((link.to_screen, entry_x, entry_y));
    }

    None
}

/// Checks if virtual cursor has gone past any edge of the current screen.
/// If a link exists on that exit side, returns the target screen index.
/// If no link exists on that side, returns None (cursor stays clamped).
pub fn check_virtual_cursor_exit(
    screens: &[ScreenConfig],
    links: &[EdgeLink],
    current_screen: usize,
    virtual_x: f64,
    virtual_y: f64,
) -> Option<usize> {
    let screen = &screens[current_screen];
    let rw = screen.real_width as f64;
    let rh = screen.real_height as f64;

    // Determine which side(s) the cursor has exited
    let exit_side = if virtual_x < 0.0 {
        Some(Side::Left)
    } else if virtual_x >= rw {
        Some(Side::Right)
    } else if virtual_y < 0.0 {
        Some(Side::Top)
    } else if virtual_y >= rh {
        Some(Side::Bottom)
    } else {
        None
    };

    if let Some(side) = exit_side {
        // Find a link on that side
        for link in links {
            if link.from_screen == current_screen && link.from_side == side {
                return Some(link.to_screen);
            }
        }
    }

    None
}

/// For the dragged screen, finds the nearest edge snap on all 4 sides.
/// Also snaps vertical/horizontal alignment (top-to-top, bottom-to-bottom, etc.).
/// Returns the snapped position.
pub fn snap_screen_position(
    screens: &[ScreenConfig],
    dragged_idx: usize,
    drag_x: f32,
    drag_y: f32,
    snap_threshold: f32,
) -> (f32, f32) {
    let dragged = &screens[dragged_idx];
    let dw = dragged.width;
    let dh = dragged.height;

    let mut best_x = drag_x;
    let mut best_y = drag_y;
    let mut best_dx = snap_threshold + 1.0;
    let mut best_dy = snap_threshold + 1.0;

    for (i, other) in screens.iter().enumerate() {
        if i == dragged_idx {
            continue;
        }

        let ow = other.width;
        let oh = other.height;

        // Horizontal snaps (x-axis)
        let x_snaps = [
            // Right edge of dragged to left edge of other
            (other.x - dw, (drag_x + dw - other.x).abs()),
            // Left edge of dragged to right edge of other
            (other.x + ow, (drag_x - (other.x + ow)).abs()),
            // Left edge to left edge (alignment)
            (other.x, (drag_x - other.x).abs()),
            // Right edge to right edge (alignment)
            (other.x + ow - dw, (drag_x + dw - (other.x + ow)).abs()),
        ];

        for (snap_val, dist) in x_snaps {
            if dist <= snap_threshold && dist < best_dx {
                best_dx = dist;
                best_x = snap_val;
            }
        }

        // Vertical snaps (y-axis)
        let y_snaps = [
            // Bottom edge of dragged to top edge of other
            (other.y - dh, (drag_y + dh - other.y).abs()),
            // Top edge of dragged to bottom edge of other
            (other.y + oh, (drag_y - (other.y + oh)).abs()),
            // Top edge to top edge (alignment)
            (other.y, (drag_y - other.y).abs()),
            // Bottom edge to bottom edge (alignment)
            (other.y + oh - dh, (drag_y + dh - (other.y + oh)).abs()),
        ];

        for (snap_val, dist) in y_snaps {
            if dist <= snap_threshold && dist < best_dy {
                best_dy = dist;
                best_y = snap_val;
            }
        }
    }

    (best_x, best_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ScreenConfig;

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
    fn test_vertical_overlap_full() {
        let a = make_screen("A", 0.0, 0.0, 200.0, 130.0, 1440, 900, true);
        let b = make_screen("B", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false);
        let (s, e) = vertical_overlap(&a, &b);
        assert!((s - 0.0).abs() < 0.001);
        assert!((e - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_vertical_overlap_partial() {
        let a = make_screen("A", 0.0, 0.0, 200.0, 100.0, 1440, 900, true);
        let b = make_screen("B", 200.0, 50.0, 200.0, 100.0, 1920, 1080, false);
        let (s, e) = vertical_overlap(&a, &b);
        assert!((s - 0.5).abs() < 0.001);
        assert!((e - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_vertical_overlap_none() {
        let a = make_screen("A", 0.0, 0.0, 200.0, 100.0, 1440, 900, true);
        let b = make_screen("B", 200.0, 200.0, 200.0, 100.0, 1920, 1080, false);
        let (s, e) = vertical_overlap(&a, &b);
        assert!((s - 0.0).abs() < 0.001);
        assert!((e - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_horizontal_overlap_full() {
        let a = make_screen("A", 0.0, 0.0, 200.0, 130.0, 1440, 900, true);
        let b = make_screen("B", 0.0, 130.0, 200.0, 130.0, 1920, 1080, false);
        let (s, e) = horizontal_overlap(&a, &b);
        assert!((s - 0.0).abs() < 0.001);
        assert!((e - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_snap_position_right_edge() {
        let screens = vec![
            make_screen("A", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
            make_screen("B", 205.0, 0.0, 200.0, 130.0, 1920, 1080, false),
        ];
        let (sx, sy) = snap_screen_position(&screens, 1, 205.0, 0.0, 20.0);
        assert!((sx - 200.0).abs() < 0.001);
        assert!((sy - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_edge_links_right_left() {
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
    fn test_compute_edge_links_far_apart() {
        let screens = vec![
            make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
            make_screen("Win", 300.0, 0.0, 200.0, 130.0, 1920, 1080, false),
        ];
        let links = compute_edge_links(&screens, 20.0);
        assert_eq!(links.len(), 0);
    }
}
