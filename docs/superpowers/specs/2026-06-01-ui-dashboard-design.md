# UI Dashboard — Design Spec

**Date:** 2026-06-01
**Status:** Approved
**Scope:** Full dashboard app with egui, system tray, connection manager, screen layout editor, event log, settings

## Goal

Replace the CLI-only server/client with a single egui desktop app that serves as both server and client. The app provides a dashboard with connection management, drag-and-drop screen layout editor, live event log, and system tray integration. Same binary runs on macOS and Windows with identical UI.

## Framework

- **egui** via `eframe` — pure Rust immediate-mode GUI, identical appearance on Mac and Windows
- **tray-icon** crate — cross-platform system tray icon with right-click menu
- Single binary, single process — UI, networking, and capture all run in the same process on separate threads

## Main Window Layout

Three vertically stacked zones:

### Status Bar (top)
- App name: "Deskserver"
- Connection indicator: green dot + IP when connected, red dot when disconnected
- Mode badge: "LOCAL" or "REMOTE"
- Latency display (ms)
- Role: "Server" or "Client"

### Tab Area (middle)
Three tabs:

**1. Screen Layout Tab**
- Drag-and-drop editor with screen rectangles
- Each screen shows: machine name, resolution, role (Server/Client), status color
- Screens can be arranged left/right/above/below
- Directional link arrows between adjacent screens
- Status text: "Drag screens to rearrange"

**2. Connections Tab**

*Server view:*
- Header: serving IP/port, client count
- List of connected clients, each showing:
  - Machine name (auto-detected from hostname)
  - IP address, connection duration
  - Screen resolution, latency
  - Status badge: ACTIVE (being controlled) or IDLE
  - Disconnect button
- Footer: "Clients auto-connect when they discover this server on the LAN"

*Client view:*
- Header: connection status
- "Scan LAN" button — discovers servers via mDNS/DNS-SD
- List of discovered servers, each showing:
  - Machine name
  - IP:port
  - Connected client count, latency
  - "Connect" button
- Manual connect: IP text field + Connect button (fallback)

**3. Settings Tab**
- Machine name (text field, default: system hostname)
- Role selector: Server / Client
- Port (default 24800)
- Hotkey display + change button (default: double-tap Left Shift)
- Auto-start on boot (checkbox)
- Auto-connect to last server (checkbox, client only)

### Event Log (bottom)
- Collapsible panel
- Scrolling log of events, color-coded:
  - Green: connection events
  - Blue: mode changes
  - Orange: warnings
  - White/gray: regular events (mouse, key)
- Ring buffer: last 500 entries
- Collapse/expand toggle

## System Tray

Icon color indicates status:
- Green: connected
- Red: disconnected
- Blue: REMOTE mode active

Right-click context menu:
- Mode toggle: "Mode: LOCAL" / "Mode: REMOTE" (clickable)
- Connection info: "Connected to: [name]" or "Not connected"
- Separator
- "Open Deskserver" — show/focus main window
- "Quit" — exit app

Minimize to tray: closing the window minimizes to tray instead of quitting. Quit only via tray menu.

## Architecture

### Threading Model
1. **Main thread** — capture loop (CGEventTap on macOS requires main thread, SetWindowsHookEx needs message loop)
2. **UI thread** — egui/eframe rendering and event loop (spawned thread)
3. **Network thread** — TCP server/client, mDNS discovery, message read/write

### Shared State
`Arc<Mutex<AppState>>` shared between all threads:

```rust
struct AppState {
    // Identity
    machine_name: String,
    role: Role, // Server or Client

    // Connection
    mode: InputMode, // LOCAL or REMOTE
    connected_clients: Vec<ClientInfo>,    // server mode
    available_servers: Vec<ServerInfo>,    // client mode
    connected_server: Option<ServerInfo>,  // client mode

    // Layout
    screens: Vec<ScreenConfig>,           // position, size, name, links

    // Log
    event_log: VecDeque<LogEntry>,        // ring buffer, max 500

    // Settings
    port: u16,
    hotkey: HotkeyConfig,
    auto_start: bool,
    auto_connect: bool,
}
```

### Discovery (mDNS)
- Server advertises `_deskserver._tcp.local` via mDNS with TXT record (machine name, version, client count)
- Client scans for this service type on LAN
- Uses `mdns-sd` crate (in-process, no external dependencies)
- Manual IP entry always available as fallback

### One Binary, Two Roles
- User selects Server or Client in Settings
- Server mode: binds TCP listener, accepts clients, runs capture
- Client mode: discovers/connects to server, synthesizes input
- Can switch role without restarting (disconnect first)

## Dependencies

New crates:
- `eframe` — egui desktop app framework
- `tray-icon` — cross-platform system tray
- `mdns-sd` — mDNS/DNS-SD discovery

Existing:
- `deskserver-common` — protocol, keymap
- `enigo` — input synthesis (client mode)
- `core-graphics`, `core-foundation` — macOS capture (server mode)
- `windows` — Windows capture (server mode)

## Project Structure Change

Replace the separate `kvm-server` and `kvm-client` crates with a single `deskserver` app crate:

```
crates/
├── common/          # shared protocol + keymap (unchanged)
├── server/          # capture module (becomes a library only, no binary)
│   └── src/
│       ├── lib.rs   # pub mod capture
│       └── capture/ # macos.rs, windows.rs, mod.rs
└── app/             # NEW: the unified egui app
    ├── Cargo.toml
    └── src/
        ├── main.rs       # entry point, thread setup
        ├── ui/
        │   ├── mod.rs    # DeskserverApp impl, tab routing
        │   ├── status.rs # status bar rendering
        │   ├── layout.rs # screen layout editor (drag-and-drop)
        │   ├── connections.rs # connection manager tab
        │   ├── settings.rs   # settings tab
        │   └── log.rs    # event log panel
        ├── state.rs      # AppState, shared state types
        ├── network.rs    # TCP server/client, message handling
        ├── discovery.rs  # mDNS discovery
        └── tray.rs       # system tray setup
```

The old `kvm-server` and `kvm-client` binaries remain for now as fallback/testing, but the primary app becomes `deskserver`.

## Out of Scope
- Encryption/pairing (Phase 4 security)
- File/clipboard sharing (Phase 4)
- Multi-monitor per machine
- Themes/customization
- Auto-update

## Success Criteria
1. App launches with the dashboard window on both macOS and Windows
2. Server mode: shows serving status, accepts client connections, lists connected clients
3. Client mode: discovers servers on LAN, connects with one click, shows connection status
4. Screen layout editor: drag screens to rearrange, saves configuration
5. Event log: shows connection events, mode changes, errors
6. System tray: icon with status color, right-click menu, minimize to tray
7. Hotkey toggle works from within the app (same as CLI version)
8. Settings persist between app restarts
