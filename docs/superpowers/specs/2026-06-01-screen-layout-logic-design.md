# Screen Layout Logic — Design Spec

**Date:** 2026-06-01
**Status:** Approved
**Scope:** Edge-based cursor crossing between machines using the layout editor, plus snap-to-edge UI, virtual cursor tracking, and integration with the capture callback

## Goal

Make the screen layout editor functional — when the cursor hits an edge that's linked to a remote screen, it automatically crosses to that machine. The cursor appears on the remote screen at the proportional position. Moving back past the return edge switches back. The double-tap Left Shift hotkey continues to work as a manual override.

## Edge Crossing Logic

### Forward Crossing (Server → Client)

1. Server is in LOCAL mode, cursor is on the server screen
2. On every `MouseMove`, check if cursor position is at a linked edge
3. Edge hit detected → compute entry position using proportional mapping:
   - If leaving from right edge at 40% from top, enter left edge of target at 40% from top
   - Map proportionally based on the overlap range between the two edges
4. Switch to REMOTE mode:
   - Hide server cursor, disconnect mouse
   - Initialize virtual cursor at the remote screen's entry position
   - Send `ScreenEnter { x, y }` with absolute entry position to client
5. Client receives `ScreenEnter { x, y }`:
   - Move cursor to (x, y) absolute
   - Switch to remote_mode (use relative movement for subsequent deltas)

### Return Crossing (Client → Server)

1. Server is in REMOTE mode, tracking virtual cursor `(virtual_x, virtual_y)` on the remote screen
2. On every `MouseMove` delta, update: `virtual_x += dx`, `virtual_y += dy`
3. Check if virtual cursor has gone past any edge of the remote screen dimensions
4. If past the return edge (the edge linked back to the server):
   - Switch to LOCAL mode
   - Show server cursor, reconnect mouse
   - Send `ScreenLeave` to client
5. Client receives `ScreenLeave`:
   - Switch to local mode (stop processing mouse events)

### Hotkey Override

Double-tap Left Shift still toggles LOCAL/REMOTE independently. If activated while LOCAL, switches to REMOTE (forwarding to first linked client). If activated while REMOTE, forces back to LOCAL.

## Layout Graph Data Model

### ScreenConfig (existing, extended)

```rust
struct ScreenConfig {
    name: String,
    x: f32,          // position in layout editor canvas
    y: f32,
    width: f32,      // display resolution width (scaled for editor)
    height: f32,     // display resolution height (scaled for editor)
    is_server: bool,
    // NEW:
    real_width: u32,  // actual screen resolution
    real_height: u32,
}
```

### EdgeLink (new)

```rust
struct EdgeLink {
    from_screen: usize,
    from_side: Side,
    to_screen: usize,
    to_side: Side,
    overlap_start: f32,  // 0.0..1.0 percentage along the edge
    overlap_end: f32,
}

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}
```

### Edge Detection Rule

Two screens are linked when:
- Their edges are within 20px of each other in the layout editor (snap threshold)
- They have vertical or horizontal overlap
- The link is bidirectional (A.right → B.left AND B.left → A.right)

Edge links are recomputed automatically whenever a screen is dragged.

### Virtual Cursor

When in REMOTE mode, the server tracks:

```rust
struct VirtualCursor {
    x: f64,           // current position on remote screen
    y: f64,
    screen_width: f64, // remote screen dimensions
    screen_height: f64,
}
```

Updated on every mouse delta. Checked against screen bounds to detect return crossing.

## Snap-to-Edge in Layout Editor

### Snap Behavior

- Dragging a screen within 20px of another screen's edge → snap to touch
- Snap works on all 4 sides
- Vertical/horizontal alignment also snaps (tops align, bottoms align)
- Moving away >20px breaks the snap

### Visual Indicators

- Linked edges: green line between touching screens
- Active crossing direction: small arrow on the linked edge
- Unlinked edges: normal border color

### Edge Link Computation

On every drag frame:
1. For the dragged screen, check all 4 edges against all edges of all other screens
2. If any edge pair is within snap threshold and has overlap:
   - Snap the position so edges touch exactly
   - Create/update EdgeLink entries
3. Remove EdgeLinks for edges that are no longer within threshold

## Wire Protocol Change

```rust
// Before:
ScreenEnter,

// After:
ScreenEnter { x: f64, y: f64 },
```

`x, y` is the absolute cursor position where the client should place its cursor on entry. This requires updating `InputMsg`, the protocol tests, and both server and client code.

## Files Changed

```
crates/common/src/lib.rs           — ScreenEnter { x, y } change
tests/integration/protocol_test.rs — update ScreenEnter test
crates/app/src/state.rs            — EdgeLink, Side, VirtualCursor, real_width/height
crates/app/src/ui/layout.rs        — snap-to-edge, green linked edge rendering
crates/app/src/main.rs             — edge detection in capture callback, virtual cursor tracking
crates/server/src/main.rs          — same edge detection (CLI server)
crates/server/src/bin/test_server.rs — keep in sync
crates/client/src/main.rs          — handle ScreenEnter { x, y }, absolute positioning on entry
```

## Out of Scope

- Multi-client edge crossing (cursor goes from client A to client B)
- DPI/scaling normalization
- Corner dead zones, dwell time, double-bump guards
- Saving/loading layout configuration to file

## Success Criteria

1. Drag Windows screen to the right of Mac screen in the layout editor → edges snap together, green link shown
2. Move Mac cursor to the right edge → cursor automatically appears on Windows at proportional position
3. Move Windows cursor (via forwarded deltas) to the left edge → cursor returns to Mac
4. Double-tap Left Shift still works as manual override in both directions
5. Layout editor accurately reflects which edges are linked
