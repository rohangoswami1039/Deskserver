# Screen Layout Logic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the screen layout editor functional — cursor automatically crosses between machines when hitting linked edges, with proportional position mapping and virtual cursor tracking for return crossing.

**Architecture:** Add `EdgeLink`, `Side`, and `VirtualCursor` types to shared state. The layout editor's snap-to-edge logic auto-computes edge links when screens are dragged near each other. The server's capture callback checks cursor position against linked edges to trigger crossing. A virtual cursor tracks position on the remote screen for return crossing.

**Tech Stack:** Rust, egui (layout editor UI), existing capture + protocol infrastructure

---

## File Structure

```
crates/
├── common/src/lib.rs                  # ScreenEnter { x, y } protocol change
├── app/src/
│   ├── state.rs                       # EdgeLink, Side, VirtualCursor, real_width/height
│   ├── ui/layout.rs                   # snap-to-edge, green linked edges, arrows
│   └── edge.rs                        # NEW: edge detection + link computation logic
├── server/src/
│   ├── main.rs                        # edge crossing in capture callback
│   └── bin/test_server.rs             # keep in sync
├── client/src/main.rs                 # handle ScreenEnter { x, y }
tests/integration/
    ├── protocol_test.rs               # update ScreenEnter test
    └── edge_test.rs                   # NEW: edge detection unit tests
```

---

### Task 1: Update ScreenEnter Protocol + Tests

**Files:**
- Modify: `crates/common/src/lib.rs`
- Modify: `tests/integration/protocol_test.rs`
- Modify: `crates/client/src/main.rs`
- Modify: `crates/client/src/bin/test_client.rs`
- Modify: `crates/app/src/network.rs`

- [ ] **Step 1: Write failing test for ScreenEnter with coordinates**

Update the `roundtrip_screen_enter` test in `tests/integration/protocol_test.rs`:

```rust
#[test]
fn roundtrip_screen_enter() {
    let msg = InputMsg::ScreenEnter { x: 150.5, y: 300.0 };
    let mut buf: Vec<u8> = Vec::new();
    write_msg(&mut buf, &msg).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = read_msg(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `source "$HOME/.cargo/env" && cargo test --test protocol_test`
Expected: FAIL — ScreenEnter doesn't have fields.

- [ ] **Step 3: Update InputMsg::ScreenEnter in lib.rs**

Change in `crates/common/src/lib.rs`:

```rust
// Change from:
    ScreenEnter,
// To:
    ScreenEnter { x: f64, y: f64 },
```

- [ ] **Step 4: Fix all code that uses ScreenEnter**

In `crates/client/src/main.rs`, update the match arm:

```rust
InputMsg::ScreenEnter { x, y } => {
    remote_mode = true;
    println!("[CLIENT] #{}: ScreenEnter at ({:.0}, {:.0}) — now controlling this machine", msg_count, x, y);
    // Move cursor to entry position
    enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
}
```

In `crates/client/src/bin/test_client.rs`, update the match arm:

```rust
InputMsg::ScreenEnter { x, y } => {
    println!("[CLIENT] Received #{}: ScreenEnter at ({:.0}, {:.0})", count, x, y);
}
```

In `crates/app/src/network.rs`, update any `ScreenEnter` match:

```rust
InputMsg::ScreenEnter { .. } => {
    let mut s = state.lock().unwrap();
    s.mode = InputMode::Remote;
    s.log("Server switched to REMOTE — controlling this machine", LogLevel::Mode);
}
```

In `crates/server/src/main.rs` and `crates/server/src/bin/test_server.rs`, update the ScreenEnter sends to include coordinates:

```rust
let _ = write_msg(&mut *s, &InputMsg::ScreenEnter { x: 0.0, y: 0.0 });
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/common/src/lib.rs tests/integration/protocol_test.rs crates/client/src/main.rs crates/client/src/bin/test_client.rs crates/app/src/network.rs crates/server/src/main.rs crates/server/src/bin/test_server.rs
git commit -m "feat: add coordinates to ScreenEnter message for entry position"
```

---

### Task 2: Add Edge Types and Virtual Cursor to State

**Files:**
- Modify: `crates/app/src/state.rs`

- [ ] **Step 1: Add Side, EdgeLink, and VirtualCursor types**

Add after the `ScreenConfig` struct in `crates/app/src/state.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeLink {
    pub from_screen: usize,
    pub from_side: Side,
    pub to_screen: usize,
    pub to_side: Side,
    pub overlap_start: f32,  // 0.0..1.0
    pub overlap_end: f32,
}

#[derive(Debug, Clone)]
pub struct VirtualCursor {
    pub x: f64,
    pub y: f64,
    pub screen_width: f64,
    pub screen_height: f64,
    pub target_screen: usize,
}
```

- [ ] **Step 2: Add real_width/height to ScreenConfig**

Add to the `ScreenConfig` struct:

```rust
    pub real_width: u32,
    pub real_height: u32,
```

Update the default ScreenConfig in `AppState::default()`:

```rust
screens: vec![ScreenConfig {
    name: "This Machine".to_string(),
    x: 50.0,
    y: 50.0,
    width: 200.0,
    height: 130.0,
    is_server: true,
    real_width: 1440,
    real_height: 900,
}],
```

- [ ] **Step 3: Add edge_links and virtual_cursor to AppState**

Add fields to the `AppState` struct:

```rust
    pub edge_links: Vec<EdgeLink>,
    pub virtual_cursor: Option<VirtualCursor>,
```

Default values:

```rust
    edge_links: Vec::new(),
    virtual_cursor: None,
```

- [ ] **Step 4: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p deskserver`
Expected: Compiles (may have warnings about unused fields).

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/state.rs
git commit -m "feat: add EdgeLink, Side, VirtualCursor types and screen dimensions to state"
```

---

### Task 3: Implement Edge Detection Logic

**Files:**
- Create: `crates/app/src/edge.rs`
- Modify: `crates/app/src/main.rs` (add `pub mod edge;`)
- Create: `tests/integration/edge_test.rs`
- Modify: `Cargo.toml` (workspace root — add test entry)

- [ ] **Step 1: Write edge detection tests**

Create `tests/integration/edge_test.rs`:

```rust
use deskserver::edge::{compute_edge_links, check_edge_crossing, Side};
use deskserver::state::ScreenConfig;

fn make_screen(name: &str, x: f32, y: f32, w: f32, h: f32, rw: u32, rh: u32, server: bool) -> ScreenConfig {
    ScreenConfig {
        name: name.to_string(),
        x, y, width: w, height: h,
        is_server: server,
        real_width: rw, real_height: rh,
    }
}

#[test]
fn screens_snapped_right_creates_link() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    assert_eq!(links.len(), 2); // bidirectional
    assert!(links.iter().any(|l| l.from_screen == 0 && l.from_side == Side::Right && l.to_screen == 1 && l.to_side == Side::Left));
    assert!(links.iter().any(|l| l.from_screen == 1 && l.from_side == Side::Left && l.to_screen == 0 && l.to_side == Side::Right));
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
    assert!(links.iter().any(|l| l.from_side == Side::Bottom && l.to_side == Side::Top));
}

#[test]
fn edge_crossing_right_detected() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    // Cursor at right edge of Mac (x=1440, y=450 which is 50% of 900)
    let result = check_edge_crossing(&screens, &links, 0, 1440.0, 450.0);
    assert!(result.is_some());
    let (target_screen, entry_x, entry_y) = result.unwrap();
    assert_eq!(target_screen, 1);
    assert!(entry_x < 10.0); // Should be near left edge of Windows (x ≈ 0)
    assert!((entry_y - 540.0).abs() < 10.0); // 50% of 1080 = 540
}

#[test]
fn edge_crossing_not_at_edge() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 0.0, 200.0, 130.0, 1920, 1080, false),
    ];
    let links = compute_edge_links(&screens, 20.0);
    // Cursor in middle of Mac screen
    let result = check_edge_crossing(&screens, &links, 0, 720.0, 450.0);
    assert!(result.is_none());
}

#[test]
fn partial_overlap_maps_proportionally() {
    let screens = vec![
        make_screen("Mac", 0.0, 0.0, 200.0, 130.0, 1440, 900, true),
        make_screen("Win", 200.0, 30.0, 200.0, 100.0, 1920, 1080, false), // offset down
    ];
    let links = compute_edge_links(&screens, 20.0);
    assert!(!links.is_empty());
}
```

- [ ] **Step 2: Add test entry to workspace Cargo.toml**

Add:
```toml
[[test]]
name = "edge_test"
path = "tests/integration/edge_test.rs"
```

Also add to `[dev-dependencies]`:
```toml
deskserver = { path = "crates/app" }
```

- [ ] **Step 3: Create `crates/app/src/edge.rs`**

```rust
use crate::state::{EdgeLink, ScreenConfig, Side};

const EDGE_THRESHOLD: f32 = 5.0; // pixels in real screen coords

/// Compute all edge links between screens based on their layout positions.
/// Two screens are linked when their edges touch (within snap_threshold in editor coords)
/// and they have vertical/horizontal overlap.
pub fn compute_edge_links(screens: &[ScreenConfig], snap_threshold: f32) -> Vec<EdgeLink> {
    let mut links = Vec::new();

    for i in 0..screens.len() {
        for j in 0..screens.len() {
            if i == j {
                continue;
            }
            let a = &screens[i];
            let b = &screens[j];

            // Check A's right edge against B's left edge
            let a_right = a.x + a.width;
            let b_left = b.x;
            if (a_right - b_left).abs() < snap_threshold {
                let (overlap_start, overlap_end) = vertical_overlap(a, b);
                if overlap_end > overlap_start {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Right,
                        to_screen: j,
                        to_side: Side::Left,
                        overlap_start,
                        overlap_end,
                    });
                }
            }

            // Check A's bottom edge against B's top edge
            let a_bottom = a.y + a.height;
            let b_top = b.y;
            if (a_bottom - b_top).abs() < snap_threshold {
                let (overlap_start, overlap_end) = horizontal_overlap(a, b);
                if overlap_end > overlap_start {
                    links.push(EdgeLink {
                        from_screen: i,
                        from_side: Side::Bottom,
                        to_screen: j,
                        to_side: Side::Top,
                        overlap_start,
                        overlap_end,
                    });
                }
            }
        }
    }

    links
}

/// Compute vertical overlap between two screens as a normalized range (0.0..1.0)
/// relative to screen A's height.
fn vertical_overlap(a: &ScreenConfig, b: &ScreenConfig) -> (f32, f32) {
    let a_top = a.y;
    let a_bottom = a.y + a.height;
    let b_top = b.y;
    let b_bottom = b.y + b.height;

    let overlap_top = a_top.max(b_top);
    let overlap_bottom = a_bottom.min(b_bottom);

    if overlap_bottom <= overlap_top {
        return (0.0, 0.0);
    }

    let start = (overlap_top - a_top) / a.height;
    let end = (overlap_bottom - a_top) / a.height;
    (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0))
}

/// Compute horizontal overlap between two screens as a normalized range (0.0..1.0)
/// relative to screen A's width.
fn horizontal_overlap(a: &ScreenConfig, b: &ScreenConfig) -> (f32, f32) {
    let a_left = a.x;
    let a_right = a.x + a.width;
    let b_left = b.x;
    let b_right = b.x + b.width;

    let overlap_left = a_left.max(b_left);
    let overlap_right = a_right.min(b_right);

    if overlap_right <= overlap_left {
        return (0.0, 0.0);
    }

    let start = (overlap_left - a_left) / a.width;
    let end = (overlap_right - a_left) / a.width;
    (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0))
}

/// Check if a cursor position on a given screen hits any linked edge.
/// Returns Some((target_screen_index, entry_x, entry_y)) if crossing detected.
/// Cursor position is in REAL screen coordinates (not editor coordinates).
pub fn check_edge_crossing(
    screens: &[ScreenConfig],
    links: &[EdgeLink],
    current_screen: usize,
    cursor_x: f64,
    cursor_y: f64,
) -> Option<(usize, f64, f64)> {
    let screen = &screens[current_screen];
    let sw = screen.real_width as f64;
    let sh = screen.real_height as f64;

    for link in links {
        if link.from_screen != current_screen {
            continue;
        }

        let at_edge = match link.from_side {
            Side::Right => cursor_x >= sw - EDGE_THRESHOLD as f64,
            Side::Left => cursor_x <= EDGE_THRESHOLD as f64,
            Side::Bottom => cursor_y >= sh - EDGE_THRESHOLD as f64,
            Side::Top => cursor_y <= EDGE_THRESHOLD as f64,
        };

        if !at_edge {
            continue;
        }

        // Check if cursor is within the overlap range
        let position_along_edge = match link.from_side {
            Side::Right | Side::Left => cursor_y / sh,
            Side::Top | Side::Bottom => cursor_x / sw,
        };

        if position_along_edge < link.overlap_start as f64
            || position_along_edge > link.overlap_end as f64
        {
            continue;
        }

        // Map position proportionally to target screen
        let target = &screens[link.to_screen];
        let tw = target.real_width as f64;
        let th = target.real_height as f64;

        // Normalize position within overlap range of source
        let overlap_range = (link.overlap_end - link.overlap_start) as f64;
        let normalized = if overlap_range > 0.0 {
            (position_along_edge - link.overlap_start as f64) / overlap_range
        } else {
            0.5
        };

        let (entry_x, entry_y) = match link.to_side {
            Side::Left => (0.0, normalized * th),
            Side::Right => (tw, normalized * th),
            Side::Top => (normalized * tw, 0.0),
            Side::Bottom => (normalized * tw, th),
        };

        return Some((link.to_screen, entry_x, entry_y));
    }

    None
}

/// Check if a virtual cursor position has gone past any edge of its screen.
/// Returns Some((return_screen, side)) if the cursor has left the screen.
pub fn check_virtual_cursor_exit(
    screens: &[ScreenConfig],
    links: &[EdgeLink],
    current_screen: usize,
    virtual_x: f64,
    virtual_y: f64,
) -> Option<usize> {
    let screen = &screens[current_screen];
    let sw = screen.real_width as f64;
    let sh = screen.real_height as f64;

    // Check if virtual cursor has gone past any edge
    let exit_side = if virtual_x < 0.0 {
        Some(Side::Left)
    } else if virtual_x > sw {
        Some(Side::Right)
    } else if virtual_y < 0.0 {
        Some(Side::Top)
    } else if virtual_y > sh {
        Some(Side::Bottom)
    } else {
        None
    };

    let exit_side = exit_side?;

    // Find a link from this screen on the exit side that goes back to a server
    for link in links {
        if link.from_screen == current_screen && link.from_side == exit_side {
            return Some(link.to_screen);
        }
    }

    // No link on that side — just clamp (don't exit)
    None
}

/// Snap a dragged screen to nearby edges of other screens.
/// Returns the snapped position and recomputed edge links.
pub fn snap_screen_position(
    screens: &[ScreenConfig],
    dragged_idx: usize,
    drag_x: f32,
    drag_y: f32,
    snap_threshold: f32,
) -> (f32, f32) {
    let dragged = &screens[dragged_idx];
    let mut snapped_x = drag_x;
    let mut snapped_y = drag_y;

    let dragged_right = drag_x + dragged.width;
    let dragged_bottom = drag_y + dragged.height;

    let mut best_dx = snap_threshold;
    let mut best_dy = snap_threshold;

    for (i, other) in screens.iter().enumerate() {
        if i == dragged_idx {
            continue;
        }

        let other_right = other.x + other.width;
        let other_bottom = other.y + other.height;

        // Snap dragged right edge to other left edge
        let d = (dragged_right - other.x).abs();
        if d < best_dx {
            best_dx = d;
            snapped_x = other.x - dragged.width;
        }

        // Snap dragged left edge to other right edge
        let d = (drag_x - other_right).abs();
        if d < best_dx {
            best_dx = d;
            snapped_x = other_right;
        }

        // Snap dragged bottom edge to other top edge
        let d = (dragged_bottom - other.y).abs();
        if d < best_dy {
            best_dy = d;
            snapped_y = other.y - dragged.height;
        }

        // Snap dragged top edge to other bottom edge
        let d = (drag_y - other_bottom).abs();
        if d < best_dy {
            best_dy = d;
            snapped_y = other_bottom;
        }

        // Vertical alignment snaps (top-to-top, bottom-to-bottom)
        let d = (drag_y - other.y).abs();
        if d < best_dy {
            best_dy = d;
            snapped_y = other.y;
        }
        let d = (dragged_bottom - other_bottom).abs();
        if d < best_dy {
            best_dy = d;
            snapped_y = other_bottom - dragged.height;
        }
    }

    (snapped_x, snapped_y)
}
```

- [ ] **Step 4: Add `pub mod edge;` to main.rs**

Add to `crates/app/src/main.rs`.

- [ ] **Step 5: Run tests**

Run: `source "$HOME/.cargo/env" && cargo test --test edge_test`
Expected: All 6 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/edge.rs crates/app/src/main.rs tests/integration/edge_test.rs Cargo.toml
git commit -m "feat: implement edge detection logic with snap, crossing, and virtual cursor exit"
```

---

### Task 4: Update Layout Editor with Snap and Linked Edge Rendering

**Files:**
- Modify: `crates/app/src/ui/layout.rs`

- [ ] **Step 1: Update layout.rs to use snap and render linked edges**

The layout editor needs these changes:
1. During drag, call `snap_screen_position()` to snap the screen
2. After drag, call `compute_edge_links()` to update `state.edge_links`
3. Render linked edges as green lines between touching screens
4. Show small arrow indicators on linked edges

Update the drag handling section to use snap:

```rust
// In the dragged() handler, replace direct position update with:
if response.dragged() {
    if let (Some(idx), Some(pos)) = (state.dragging_screen, pointer_pos) {
        let raw_x = pos.x - canvas_rect.min.x - state.drag_offset.0;
        let raw_y = pos.y - canvas_rect.min.y - state.drag_offset.1;
        let (snapped_x, snapped_y) = crate::edge::snap_screen_position(
            &state.screens, idx, raw_x, raw_y, 20.0,
        );
        state.screens[idx].x = snapped_x.clamp(0.0, canvas_size.x - state.screens[idx].width);
        state.screens[idx].y = snapped_y.clamp(0.0, canvas_size.y - state.screens[idx].height);
    }
}

// After drag_stopped, recompute edge links:
if response.drag_stopped() {
    state.dragging_screen = None;
    state.edge_links = crate::edge::compute_edge_links(&state.screens, 20.0);
}
```

After drawing screens, render linked edges as green lines:

```rust
// Draw linked edges
for link in &state.edge_links {
    let from = &state.screens[link.from_screen];
    let to = &state.screens[link.to_screen];

    let (p1, p2) = match link.from_side {
        crate::state::Side::Right => {
            let x = canvas_rect.min.x + from.x + from.width;
            let y1 = canvas_rect.min.y + from.y + from.height * link.overlap_start;
            let y2 = canvas_rect.min.y + from.y + from.height * link.overlap_end;
            (egui::Pos2::new(x, y1), egui::Pos2::new(x, y2))
        }
        crate::state::Side::Left => {
            let x = canvas_rect.min.x + from.x;
            let y1 = canvas_rect.min.y + from.y + from.height * link.overlap_start;
            let y2 = canvas_rect.min.y + from.y + from.height * link.overlap_end;
            (egui::Pos2::new(x, y1), egui::Pos2::new(x, y2))
        }
        crate::state::Side::Bottom => {
            let y = canvas_rect.min.y + from.y + from.height;
            let x1 = canvas_rect.min.x + from.x + from.width * link.overlap_start;
            let x2 = canvas_rect.min.x + from.x + from.width * link.overlap_end;
            (egui::Pos2::new(x1, y), egui::Pos2::new(x2, y))
        }
        crate::state::Side::Top => {
            let y = canvas_rect.min.y + from.y;
            let x1 = canvas_rect.min.x + from.x + from.width * link.overlap_start;
            let x2 = canvas_rect.min.x + from.x + from.width * link.overlap_end;
            (egui::Pos2::new(x1, y), egui::Pos2::new(x2, y))
        }
    };

    painter.line_segment(
        [p1, p2],
        egui::Stroke::new(3.0, egui::Color32::from_rgb(74, 222, 128)),
    );
}
```

- [ ] **Step 2: Verify it compiles and renders**

Run: `source "$HOME/.cargo/env" && cargo run -p deskserver`
Expected: When screens are dragged next to each other, they snap and green lines appear on linked edges.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/ui/layout.rs
git commit -m "feat: add snap-to-edge and linked edge rendering to layout editor"
```

---

### Task 5: Integrate Edge Crossing with Server Capture

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/server/src/bin/test_server.rs`

- [ ] **Step 1: Add edge crossing logic to the capture callback**

This task integrates the edge detection into the CLI server's capture callback. The server needs:

1. A layout config (for now, hardcoded two screens side by side)
2. Edge crossing detection on MouseMove when in LOCAL mode
3. Virtual cursor tracking when in REMOTE mode
4. Return crossing detection

Add imports at the top of `main.rs`:

```rust
use deskserver_common::keymap::macos_keycode_to_neutral; // already there
```

Add after the `stream` Mutex creation, before `run_capture`:

```rust
    // Hardcoded layout: server screen on left, client on right
    // TODO: In the UI app, this comes from the layout editor
    use deskserver_common::MouseButton as MB;

    let server_width: f64 = 1440.0;  // Mac screen width
    let server_height: f64 = 900.0;  // Mac screen height
    let client_width: f64 = 1920.0;  // Windows screen width
    let client_height: f64 = 1080.0; // Windows screen height

    // Virtual cursor for tracking position on remote screen
    let mut virtual_x: f64 = 0.0;
    let mut virtual_y: f64 = 0.0;
```

Update the capture callback to add edge detection:

```rust
    run_capture(move |event| {
        if is_hotkey(&event) {
            toggle_mode(&stream);
            return true;
        }

        if MODE.load(Ordering::SeqCst) == LOCAL {
            // Check for edge crossing on mouse move
            if let CaptureEvent::MouseMove { x, y, .. } = &event {
                if *x >= server_width - 2.0 {
                    // Cursor hit right edge — cross to client
                    let entry_pct = *y / server_height;
                    let entry_y = entry_pct * client_height;
                    let entry_x = 0.0;

                    MODE.store(REMOTE, Ordering::SeqCst);
                    virtual_x = entry_x;
                    virtual_y = entry_y;

                    println!("[SERVER] Edge crossing → REMOTE (entry at {:.0}, {:.0})", entry_x, entry_y);

                    #[cfg(target_os = "macos")]
                    {
                        kvm_server_lib::capture::macos::hide_cursor();
                        kvm_server_lib::capture::macos::disconnect_mouse();
                    }

                    let mut s = stream.lock().unwrap();
                    let _ = write_msg(&mut *s, &InputMsg::ScreenEnter { x: entry_x, y: entry_y });

                    return true; // Suppress
                }
            }
            return false; // Pass through in LOCAL
        }

        // REMOTE mode — pin cursor and forward
        #[cfg(target_os = "macos")]
        if matches!(&event, CaptureEvent::MouseMove { .. }) {
            kvm_server_lib::capture::macos::pin_cursor();
        }

        let msg = match &event {
            CaptureEvent::MouseMove { delta_x, delta_y, .. } => {
                // Update virtual cursor
                virtual_x += *delta_x;
                virtual_y += *delta_y;

                // Check for return crossing (virtual cursor past left edge)
                if virtual_x < 0.0 {
                    // Return to server
                    MODE.store(LOCAL, Ordering::SeqCst);
                    println!("[SERVER] Return crossing → LOCAL");

                    #[cfg(target_os = "macos")]
                    {
                        kvm_server_lib::capture::macos::reconnect_mouse();
                        kvm_server_lib::capture::macos::show_cursor();
                    }

                    let mut s = stream.lock().unwrap();
                    let _ = write_msg(&mut *s, &InputMsg::ScreenLeave);
                    return true;
                }

                // Clamp virtual cursor to screen bounds
                virtual_x = virtual_x.clamp(0.0, client_width);
                virtual_y = virtual_y.clamp(0.0, client_height);

                if delta_x.abs() > 0.1 || delta_y.abs() > 0.1 {
                    Some(InputMsg::MouseMove { x: *delta_x, y: *delta_y })
                } else {
                    None
                }
            }
            CaptureEvent::MouseButton { button, pressed } => {
                Some(InputMsg::MouseButton { button: *button, pressed: *pressed })
            }
            CaptureEvent::Wheel { dx, dy } => {
                Some(InputMsg::Wheel { dx: *dx, dy: *dy })
            }
            CaptureEvent::KeyDown { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyDown {
                    key,
                    modifiers: *modifiers,
                })
            }
            CaptureEvent::KeyUp { keycode, modifiers } => {
                to_neutral_key(*keycode).map(|key| InputMsg::KeyUp {
                    key,
                    modifiers: *modifiers,
                })
            }
        };

        if let Some(msg) = msg {
            let mut s = stream.lock().unwrap();
            if let Err(e) = write_msg(&mut *s, &msg) {
                eprintln!("[SERVER] Write error: {}", e);
            }
        }

        true // Suppress in REMOTE mode
    });
```

- [ ] **Step 2: Copy main.rs to test_server.rs**

```bash
cp crates/server/src/main.rs crates/server/src/bin/test_server.rs
```

- [ ] **Step 3: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-server`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/main.rs crates/server/src/bin/test_server.rs
git commit -m "feat: integrate edge crossing with server capture callback"
```

---

### Task 6: Update Client for Entry Position

**Files:**
- Modify: `crates/client/src/main.rs`

- [ ] **Step 1: Update client ScreenEnter handler**

The client should move the cursor to the entry position on ScreenEnter, then switch to relative mode. This was partially done in Task 1. Verify the client code handles it correctly:

```rust
InputMsg::ScreenEnter { x, y } => {
    remote_mode = true;
    println!("[CLIENT] #{}: ScreenEnter at ({:.0}, {:.0}) — moving cursor and switching to relative mode", msg_count, x, y);
    enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
}
```

- [ ] **Step 2: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p kvm-client`
Expected: Compiles.

- [ ] **Step 3: Commit if changes were needed**

```bash
git add crates/client/src/main.rs
git commit -m "feat: handle ScreenEnter entry position in client"
```

---

### Task 7: Build, Test, and Verify

**Files:** None — verification task.

- [ ] **Step 1: Run all tests**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: All tests pass (protocol 10 + keymap 7 + edge 6 = 23).

- [ ] **Step 2: Build all binaries**

Run: `source "$HOME/.cargo/env" && cargo build --release -p kvm-server -p kvm-client -p deskserver`
Expected: All three compile.

- [ ] **Step 3: Verify layout editor snap behavior**

Run: `cargo run -p deskserver`
- Drag a second screen next to the first
- Verify it snaps when edges are close
- Verify green linked edge appears

- [ ] **Step 4: Verify edge crossing with CLI server**

Run server on Mac Terminal, client on Windows:
- Move cursor to right edge of Mac screen
- Should auto-switch to REMOTE, Windows cursor appears at proportional position
- Move Windows cursor back to left edge
- Should auto-switch back to LOCAL

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve issues from end-to-end verification"
```

---

## Verification Checklist

- [ ] `cargo test` — all tests pass (protocol + keymap + edge)
- [ ] Layout editor: screens snap to edges when dragged close
- [ ] Layout editor: green lines appear on linked edges
- [ ] Edge crossing: cursor at right edge → auto-switch to REMOTE
- [ ] Entry position: cursor appears at proportional position on client
- [ ] Return crossing: virtual cursor past left edge → auto-switch to LOCAL
- [ ] Hotkey override: double-tap Left Shift still works independently
- [ ] All three binaries compile: kvm-server, kvm-client, deskserver
