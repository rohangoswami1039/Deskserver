# UI Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a unified egui desktop app ("Deskserver") that replaces the separate CLI server/client with a full dashboard — connection manager, screen layout editor, event log, settings, and system tray.

**Architecture:** Single `crates/app` crate using `eframe` (egui). The existing `kvm_server_lib` capture module is reused as a library dependency. Networking and capture run on background threads, sharing state with the UI via `Arc<Mutex<AppState>>`. The `tray-icon` crate provides cross-platform system tray.

**Tech Stack:** Rust, eframe/egui (UI), tray-icon (system tray), mdns-sd (discovery), tokio or std threads (networking), serde + bincode (protocol — existing)

---

## File Structure

```
crates/
├── common/                    # UNCHANGED — protocol + keymap
├── server/                    # UNCHANGED — capture module (library only)
│   └── src/
│       ├── lib.rs             # pub mod capture
│       └── capture/           # macos.rs, windows.rs, mod.rs
└── app/                       # NEW — the unified egui app
    ├── Cargo.toml
    └── src/
        ├── main.rs            # entry point: spawn threads, run UI
        ├── state.rs           # AppState struct, Role, InputMode, shared types
        ├── network.rs         # TCP server/client logic, message send/receive
        ├── discovery.rs       # mDNS service advertisement + browsing
        ├── tray.rs            # system tray icon + menu setup
        └── ui/
            ├── mod.rs         # DeskserverApp struct, eframe::App impl, tab routing
            ├── status.rs      # top status bar rendering
            ├── layout.rs      # screen layout drag-and-drop editor
            ├── connections.rs # connections tab (server: client list, client: server list)
            ├── settings.rs    # settings form
            └── log.rs         # event log panel
Cargo.toml                     # workspace root — add "crates/app" to members
```

---

### Task 1: Scaffold the App Crate with egui Window

**Files:**
- Modify: `Cargo.toml` (workspace root — add `crates/app` to members)
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/main.rs`
- Create: `crates/app/src/state.rs`
- Create: `crates/app/src/ui/mod.rs`

- [ ] **Step 1: Add app to workspace members**

In the root `Cargo.toml`, change the members line:

```toml
members = ["crates/common", "crates/server", "crates/client", "crates/app"]
```

- [ ] **Step 2: Create `crates/app/Cargo.toml`**

```toml
[package]
name = "deskserver"
version = "0.1.0"
edition = "2021"

[dependencies]
deskserver-common = { path = "../common" }
kvm_server_lib = { path = "../server" }
eframe = "0.31"
egui = "0.31"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 3: Create `crates/app/src/state.rs`**

```rust
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    Server,
    Client,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Local,
    Remote,
}

#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub name: String,
    pub ip: String,
    pub resolution: String,
    pub latency_ms: u32,
    pub connected_at: String,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub addr: String,
    pub client_count: u32,
    pub latency_ms: u32,
}

#[derive(Clone, Debug)]
pub struct ScreenConfig {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub is_server: bool,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Clone, Debug)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Mode,
}

pub struct AppState {
    // Identity
    pub machine_name: String,
    pub role: Role,

    // Connection
    pub mode: InputMode,
    pub connected_clients: Vec<ClientInfo>,
    pub available_servers: Vec<ServerInfo>,
    pub connected_server: Option<ServerInfo>,

    // Layout
    pub screens: Vec<ScreenConfig>,

    // Log
    pub event_log: VecDeque<LogEntry>,

    // Settings
    pub port: u16,
    pub auto_start: bool,
    pub auto_connect: bool,

    // UI state
    pub active_tab: usize,
    pub log_collapsed: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        Self {
            machine_name: hostname,
            role: Role::Server,
            mode: InputMode::Local,
            connected_clients: Vec::new(),
            available_servers: Vec::new(),
            connected_server: None,
            screens: vec![
                ScreenConfig {
                    name: "This Machine".to_string(),
                    x: 50.0, y: 80.0,
                    width: 160.0, height: 100.0,
                    is_server: true,
                },
            ],
            event_log: VecDeque::new(),
            port: 24800,
            auto_start: false,
            auto_connect: false,
            active_tab: 0,
            log_collapsed: false,
        }
    }
}

impl AppState {
    pub fn log(&mut self, level: LogLevel, message: &str) {
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        self.event_log.push_back(LogEntry {
            timestamp: now,
            message: message.to_string(),
            level,
        });
        if self.event_log.len() > 500 {
            self.event_log.pop_front();
        }
    }
}
```

- [ ] **Step 4: Create `crates/app/src/ui/mod.rs`**

```rust
pub mod status;
pub mod layout;
pub mod connections;
pub mod settings;
pub mod log;

use std::sync::{Arc, Mutex};
use crate::state::AppState;

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
        // Request repaint every 100ms to show live updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        let mut state = self.state.lock().unwrap();

        // Top status bar
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            status::render(ui, &state);
        });

        // Bottom event log
        if !state.log_collapsed {
            egui::TopBottomPanel::bottom("event_log")
                .resizable(true)
                .min_height(60.0)
                .default_height(120.0)
                .show(ctx, |ui| {
                    log::render(ui, &mut state);
                });
        } else {
            egui::TopBottomPanel::bottom("log_toggle").exact_height(24.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("▲ Show Log").clicked() {
                        state.log_collapsed = false;
                    }
                });
            });
        }

        // Central panel with tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.active_tab, 0, "Screen Layout");
                ui.selectable_value(&mut state.active_tab, 1, "Connections");
                ui.selectable_value(&mut state.active_tab, 2, "Settings");
            });
            ui.separator();

            match state.active_tab {
                0 => layout::render(ui, &mut state),
                1 => connections::render(ui, &mut state),
                2 => settings::render(ui, &mut state),
                _ => {}
            }
        });
    }
}
```

- [ ] **Step 5: Create stub UI modules**

Create `crates/app/src/ui/status.rs`:
```rust
use crate::state::{AppState, InputMode, Role};

pub fn render(ui: &mut egui::Ui, state: &AppState) {
    ui.horizontal(|ui| {
        ui.heading("Deskserver");
        ui.separator();

        // Connection status
        let (color, text) = if state.connected_server.is_some() || !state.connected_clients.is_empty() {
            (egui::Color32::from_rgb(74, 222, 128), "Connected")
        } else {
            (egui::Color32::from_rgb(248, 113, 113), "Disconnected")
        };
        let dot = egui::RichText::new("●").color(color).size(10.0);
        ui.label(dot);
        ui.label(text);

        ui.separator();

        // Mode
        let mode_text = match state.mode {
            InputMode::Local => egui::RichText::new("LOCAL").color(egui::Color32::from_rgb(125, 211, 252)),
            InputMode::Remote => egui::RichText::new("REMOTE").color(egui::Color32::from_rgb(250, 204, 21)),
        };
        ui.label(mode_text);

        ui.separator();

        // Role
        let role_text = match state.role {
            Role::Server => "Server",
            Role::Client => "Client",
        };
        ui.label(role_text);
    });
}
```

Create `crates/app/src/ui/layout.rs`:
```rust
use crate::state::AppState;

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Screen Layout");
    ui.label("Drag screens to arrange them.");

    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(
        egui::Vec2::new(available.x, 250.0),
        egui::Sense::hover(),
    );

    let rect = response.rect;
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 40));

    for screen in &state.screens {
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::new(rect.min.x + screen.x, rect.min.y + screen.y),
            egui::Vec2::new(screen.width, screen.height),
        );
        let color = if screen.is_server {
            egui::Color32::from_rgb(15, 52, 96)
        } else {
            egui::Color32::from_rgb(30, 17, 69)
        };
        painter.rect_filled(screen_rect, 4.0, color);
        painter.rect_stroke(screen_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(125, 211, 252)));
        painter.text(
            screen_rect.center(),
            egui::Align2::CENTER_CENTER,
            &screen.name,
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
    }
}
```

Create `crates/app/src/ui/connections.rs`:
```rust
use crate::state::{AppState, Role};

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    match state.role {
        Role::Server => render_server(ui, state),
        Role::Client => render_client(ui, state),
    }
}

fn render_server(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Connected Clients");
    ui.label(format!("Serving on 0.0.0.0:{} — {} clients", state.port, state.connected_clients.len()));
    ui.separator();

    if state.connected_clients.is_empty() {
        ui.label("No clients connected. Waiting...");
    }
    for client in &state.connected_clients {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&client.name).strong());
            ui.label(&client.ip);
            ui.label(format!("{}ms", client.latency_ms));
            let status = if client.active { "ACTIVE" } else { "IDLE" };
            ui.label(status);
        });
    }
}

fn render_client(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Available Servers");

    if ui.button("↻ Scan LAN").clicked() {
        // Discovery will be wired in Task 5
    }
    ui.separator();

    if state.available_servers.is_empty() {
        ui.label("No servers found. Scan or enter IP manually.");
    }
    for server in &state.available_servers {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&server.name).strong());
            ui.label(&server.addr);
            if ui.button("Connect").clicked() {
                // Network connect will be wired in Task 4
            }
        });
    }

    ui.separator();
    ui.label("Manual connect:");
    ui.horizontal(|ui| {
        // Simple text input — will use a persistent string in state later
        ui.label("IP:");
        ui.text_edit_singleline(&mut String::new());
        if ui.button("Connect").clicked() {
            // Network connect will be wired in Task 4
        }
    });
}
```

Create `crates/app/src/ui/settings.rs`:
```rust
use crate::state::{AppState, Role};

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Settings");

    egui::Grid::new("settings_grid")
        .num_columns(2)
        .spacing([20.0, 8.0])
        .show(ui, |ui| {
            ui.label("Machine Name:");
            ui.text_edit_singleline(&mut state.machine_name);
            ui.end_row();

            ui.label("Role:");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.role, Role::Server, "Server");
                ui.selectable_value(&mut state.role, Role::Client, "Client");
            });
            ui.end_row();

            ui.label("Port:");
            let mut port_str = state.port.to_string();
            if ui.text_edit_singleline(&mut port_str).changed() {
                if let Ok(p) = port_str.parse::<u16>() {
                    state.port = p;
                }
            }
            ui.end_row();

            ui.label("Hotkey:");
            ui.label("Double-tap Left Shift");
            ui.end_row();

            ui.label("Auto-start:");
            ui.checkbox(&mut state.auto_start, "Launch on system boot");
            ui.end_row();

            ui.label("Auto-connect:");
            ui.checkbox(&mut state.auto_connect, "Connect to last server");
            ui.end_row();
        });
}
```

Create `crates/app/src/ui/log.rs`:
```rust
use crate::state::{AppState, LogLevel};

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Event Log").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("▼ Collapse").clicked() {
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
        .show(ui, |ui| {
            for entry in &state.event_log {
                let color = match entry.level {
                    LogLevel::Info => egui::Color32::from_rgb(150, 150, 150),
                    LogLevel::Success => egui::Color32::from_rgb(74, 222, 128),
                    LogLevel::Warning => egui::Color32::from_rgb(251, 191, 36),
                    LogLevel::Mode => egui::Color32::from_rgb(125, 211, 252),
                };
                ui.label(
                    egui::RichText::new(format!("[{}] {}", entry.timestamp, entry.message))
                        .color(color)
                        .size(11.0)
                        .font(egui::FontId::monospace(11.0)),
                );
            }
        });
}
```

- [ ] **Step 6: Create `crates/app/src/main.rs`**

```rust
mod state;
mod ui;

use state::{AppState, LogLevel};
use ui::DeskserverApp;
use std::sync::{Arc, Mutex};

fn main() {
    let state = Arc::new(Mutex::new(AppState::default()));

    // Add initial log entry
    state.lock().unwrap().log(LogLevel::Info, "Deskserver started");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 500.0])
            .with_min_inner_size([500.0, 350.0])
            .with_title("Deskserver"),
        ..Default::default()
    };

    let state_clone = state.clone();
    eframe::run_native(
        "Deskserver",
        options,
        Box::new(move |_cc| Ok(Box::new(DeskserverApp::new(state_clone)))),
    )
    .expect("Failed to start Deskserver");
}
```

- [ ] **Step 7: Add `hostname` and `chrono` dependencies**

Add to `crates/app/Cargo.toml` under `[dependencies]`:

```toml
hostname = "0.4"
chrono = "0.4"
```

- [ ] **Step 8: Verify it compiles and launches**

Run: `source "$HOME/.cargo/env" && cargo build -p deskserver`
Expected: Compiles. Then run `cargo run -p deskserver` — a window should appear with the Deskserver dashboard.

- [ ] **Step 9: Commit**

```bash
git add crates/app/ Cargo.toml
git commit -m "feat: scaffold egui dashboard app with status bar, tabs, layout editor, connections, settings, event log"
```

---

### Task 2: Make the Layout Editor Interactive (Drag-and-Drop)

**Files:**
- Modify: `crates/app/src/ui/layout.rs`
- Modify: `crates/app/src/state.rs` (add dragging state)

- [ ] **Step 1: Add drag state to AppState**

Add to `AppState` in `crates/app/src/state.rs`:

```rust
    // Layout editor drag state
    pub dragging_screen: Option<usize>,
    pub drag_offset: (f32, f32),
```

And in `Default`:
```rust
    dragging_screen: None,
    drag_offset: (0.0, 0.0),
```

- [ ] **Step 2: Rewrite layout.rs with interactive dragging**

Replace `crates/app/src/ui/layout.rs`:

```rust
use crate::state::AppState;

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("Screen Layout");
        ui.label(" — drag screens to arrange");
    });

    let available = ui.available_size();
    let canvas_size = egui::Vec2::new(available.x, 280.0);
    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
    let canvas_rect = response.rect;

    // Dark background
    painter.rect_filled(canvas_rect, 4.0, egui::Color32::from_rgb(15, 15, 30));

    // Grid lines
    for i in 0..=(canvas_size.x as i32 / 40) {
        let x = canvas_rect.min.x + (i * 40) as f32;
        painter.line_segment(
            [egui::Pos2::new(x, canvas_rect.min.y), egui::Pos2::new(x, canvas_rect.max.y)],
            egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 30, 50)),
        );
    }
    for i in 0..=(canvas_size.y as i32 / 40) {
        let y = canvas_rect.min.y + (i * 40) as f32;
        painter.line_segment(
            [egui::Pos2::new(canvas_rect.min.x, y), egui::Pos2::new(canvas_rect.max.x, y)],
            egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 30, 50)),
        );
    }

    let pointer_pos = response.interact_pointer_pos();

    // Handle drag start
    if response.drag_started() {
        if let Some(pos) = pointer_pos {
            for (i, screen) in state.screens.iter().enumerate() {
                let screen_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(canvas_rect.min.x + screen.x, canvas_rect.min.y + screen.y),
                    egui::Vec2::new(screen.width, screen.height),
                );
                if screen_rect.contains(pos) {
                    state.dragging_screen = Some(i);
                    state.drag_offset = (pos.x - screen_rect.min.x, pos.y - screen_rect.min.y);
                    break;
                }
            }
        }
    }

    // Handle dragging
    if response.dragged() {
        if let (Some(idx), Some(pos)) = (state.dragging_screen, pointer_pos) {
            let new_x = pos.x - canvas_rect.min.x - state.drag_offset.0;
            let new_y = pos.y - canvas_rect.min.y - state.drag_offset.1;
            state.screens[idx].x = new_x.clamp(0.0, canvas_size.x - state.screens[idx].width);
            state.screens[idx].y = new_y.clamp(0.0, canvas_size.y - state.screens[idx].height);
        }
    }

    // Handle drag end
    if response.drag_stopped() {
        state.dragging_screen = None;
    }

    // Draw screens
    for (i, screen) in state.screens.iter().enumerate() {
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::new(canvas_rect.min.x + screen.x, canvas_rect.min.y + screen.y),
            egui::Vec2::new(screen.width, screen.height),
        );

        let is_dragging = state.dragging_screen == Some(i);
        let bg = if screen.is_server {
            egui::Color32::from_rgb(15, 52, 96)
        } else {
            egui::Color32::from_rgb(30, 17, 69)
        };
        let border_color = if is_dragging {
            egui::Color32::from_rgb(250, 204, 21)
        } else if screen.is_server {
            egui::Color32::from_rgb(125, 211, 252)
        } else {
            egui::Color32::from_rgb(167, 139, 250)
        };

        painter.rect_filled(screen_rect, 6.0, bg);
        painter.rect_stroke(screen_rect, 6.0, egui::Stroke::new(2.0, border_color));

        painter.text(
            screen_rect.center() - egui::Vec2::new(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            &screen.name,
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );

        let role_text = if screen.is_server { "Server" } else { "Client" };
        let role_color = if screen.is_server {
            egui::Color32::from_rgb(74, 222, 128)
        } else {
            egui::Color32::from_rgb(245, 158, 11)
        };
        painter.text(
            screen_rect.center() + egui::Vec2::new(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            role_text,
            egui::FontId::proportional(10.0),
            role_color,
        );
    }
}
```

- [ ] **Step 3: Verify it compiles and dragging works**

Run: `source "$HOME/.cargo/env" && cargo run -p deskserver`
Expected: Screen rectangles can be dragged within the canvas.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/ui/layout.rs crates/app/src/state.rs
git commit -m "feat: add interactive drag-and-drop to screen layout editor"
```

---

### Task 3: Add Network Thread (Server + Client TCP)

**Files:**
- Create: `crates/app/src/network.rs`
- Modify: `crates/app/src/main.rs` (spawn network thread)
- Modify: `crates/app/src/state.rs` (add manual_connect_ip, network commands)
- Modify: `crates/app/src/ui/connections.rs` (wire up connect/disconnect buttons)

- [ ] **Step 1: Add network command channel to state**

Add to `crates/app/src/state.rs`:

```rust
use std::sync::mpsc;

pub enum NetworkCommand {
    StartServer { port: u16 },
    ConnectTo { addr: String },
    Disconnect,
}
```

Add to `AppState`:
```rust
    pub manual_connect_ip: String,
    pub network_tx: Option<mpsc::Sender<NetworkCommand>>,
```

And in `Default`:
```rust
    manual_connect_ip: String::new(),
    network_tx: None,
```

- [ ] **Step 2: Create `crates/app/src/network.rs`**

```rust
use crate::state::{AppState, ClientInfo, LogLevel, NetworkCommand, Role};
use deskserver_common::{read_msg, write_msg, InputMsg};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn spawn_network_thread(
    state: Arc<Mutex<AppState>>,
) -> mpsc::Sender<NetworkCommand> {
    let (tx, rx) = mpsc::channel::<NetworkCommand>();

    thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(NetworkCommand::StartServer { port }) => {
                    run_server(state.clone(), port, &rx);
                }
                Ok(NetworkCommand::ConnectTo { addr }) => {
                    run_client(state.clone(), &addr);
                }
                Ok(NetworkCommand::Disconnect) => {
                    // Handled inside run_server/run_client
                }
                Err(_) => break, // Channel closed
            }
        }
    });

    tx
}

fn run_server(state: Arc<Mutex<AppState>>, port: u16, _rx: &mpsc::Receiver<NetworkCommand>) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => {
            state.lock().unwrap().log(LogLevel::Success, &format!("Server listening on {}", addr));
            l
        }
        Err(e) => {
            state.lock().unwrap().log(LogLevel::Warning, &format!("Failed to bind: {}", e));
            return;
        }
    };
    listener.set_nonblocking(false).ok();

    state.lock().unwrap().log(LogLevel::Info, "Waiting for client...");

    match listener.accept() {
        Ok((stream, client_addr)) => {
            stream.set_nodelay(true).ok();
            let client_ip = client_addr.to_string();
            {
                let mut s = state.lock().unwrap();
                s.connected_clients.push(ClientInfo {
                    name: format!("Client-{}", s.connected_clients.len() + 1),
                    ip: client_ip.clone(),
                    resolution: "Unknown".to_string(),
                    latency_ms: 0,
                    connected_at: chrono::Local::now().format("%H:%M:%S").to_string(),
                    active: false,
                });
                s.log(LogLevel::Success, &format!("Client connected: {}", client_ip));
            }

            // Store stream for capture thread to use
            let stream_mutex = Arc::new(Mutex::new(stream));
            // Store in a thread-local or pass via state — for now just keep alive
            loop {
                thread::sleep(Duration::from_secs(1));
                // The capture thread writes to this stream
                // This thread keeps the connection alive
            }
        }
        Err(e) => {
            state.lock().unwrap().log(LogLevel::Warning, &format!("Accept failed: {}", e));
        }
    }
}

fn run_client(state: Arc<Mutex<AppState>>, addr: &str) {
    state.lock().unwrap().log(LogLevel::Info, &format!("Connecting to {}...", addr));

    let full_addr = if addr.contains(':') {
        addr.to_string()
    } else {
        format!("{}:24800", addr)
    };

    match TcpStream::connect_timeout(
        &full_addr.parse().unwrap_or_else(|_| {
            format!("{}:24800", addr).parse().expect("invalid address")
        }),
        Duration::from_secs(5),
    ) {
        Ok(stream) => {
            stream.set_nodelay(true).ok();
            {
                let mut s = state.lock().unwrap();
                s.connected_server = Some(crate::state::ServerInfo {
                    name: "Server".to_string(),
                    addr: full_addr.clone(),
                    client_count: 0,
                    latency_ms: 0,
                });
                s.log(LogLevel::Success, &format!("Connected to {}", full_addr));
            }

            // Read loop
            let mut stream = stream;
            loop {
                match read_msg(&mut stream) {
                    Ok(msg) => {
                        // Process message — synthesis will be wired later
                        match &msg {
                            InputMsg::ScreenEnter => {
                                let mut s = state.lock().unwrap();
                                s.mode = crate::state::InputMode::Remote;
                                s.log(LogLevel::Mode, "Server switched to REMOTE — controlling this machine");
                            }
                            InputMsg::ScreenLeave => {
                                let mut s = state.lock().unwrap();
                                s.mode = crate::state::InputMode::Local;
                                s.log(LogLevel::Mode, "Server switched to LOCAL — control returned");
                            }
                            _ => {} // Mouse/key events handled by synthesis
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                        state.lock().unwrap().log(LogLevel::Warning, "Server disconnected");
                        break;
                    }
                    Err(e) => {
                        state.lock().unwrap().log(LogLevel::Warning, &format!("Read error: {}", e));
                        break;
                    }
                }
            }

            state.lock().unwrap().connected_server = None;
        }
        Err(e) => {
            state.lock().unwrap().log(LogLevel::Warning, &format!("Connection failed: {}", e));
        }
    }
}
```

- [ ] **Step 3: Update main.rs to spawn network thread**

Add after creating state and before eframe::run_native:

```rust
    // Spawn network thread
    let net_tx = crate::network::spawn_network_thread(state.clone());
    state.lock().unwrap().network_tx = Some(net_tx);
```

Add `mod network;` to the top of main.rs.

- [ ] **Step 4: Wire up connections.rs buttons**

Update the connect button in `render_client`:

```rust
    ui.separator();
    ui.label("Manual connect:");
    ui.horizontal(|ui| {
        ui.label("IP:");
        ui.text_edit_singleline(&mut state.manual_connect_ip);
        if ui.button("Connect").clicked() && !state.manual_connect_ip.is_empty() {
            if let Some(tx) = &state.network_tx {
                let _ = tx.send(crate::state::NetworkCommand::ConnectTo {
                    addr: state.manual_connect_ip.clone(),
                });
            }
        }
    });
```

- [ ] **Step 5: Add `chrono` dependency if not already present**

Already added in Task 1.

- [ ] **Step 6: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p deskserver`
Expected: Compiles.

- [ ] **Step 7: Commit**

```bash
git add crates/app/src/network.rs crates/app/src/main.rs crates/app/src/state.rs crates/app/src/ui/connections.rs
git commit -m "feat: add network thread with TCP server/client and connection management UI"
```

---

### Task 4: Add System Tray

**Files:**
- Create: `crates/app/src/tray.rs`
- Modify: `crates/app/Cargo.toml` (add tray-icon dependency)
- Modify: `crates/app/src/main.rs` (spawn tray)

- [ ] **Step 1: Add dependency**

Add to `crates/app/Cargo.toml`:

```toml
tray-icon = "0.19"
```

- [ ] **Step 2: Create `crates/app/src/tray.rs`**

```rust
use crate::state::{AppState, InputMode};
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

pub struct TrayState {
    pub tray: TrayIcon,
    pub quit_id: tray_icon::menu::MenuId,
    pub open_id: tray_icon::menu::MenuId,
    pub toggle_id: tray_icon::menu::MenuId,
}

pub fn create_tray() -> TrayState {
    let menu = Menu::new();
    let toggle_item = MenuItem::new("Toggle Mode", true, None);
    let open_item = MenuItem::new("Open Deskserver", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let toggle_id = toggle_item.id().clone();
    let open_id = open_item.id().clone();
    let quit_id = quit_item.id().clone();

    menu.append(&toggle_item).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&open_item).ok();
    menu.append(&quit_item).ok();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Deskserver")
        .build()
        .expect("Failed to create tray icon");

    TrayState { tray, quit_id, open_id, toggle_id }
}

pub fn poll_tray_events(tray: &TrayState, state: &Arc<Mutex<AppState>>) -> bool {
    if let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == tray.quit_id {
            return true; // Signal quit
        }
        if event.id == tray.toggle_id {
            let mut s = state.lock().unwrap();
            s.mode = match s.mode {
                InputMode::Local => InputMode::Remote,
                InputMode::Remote => InputMode::Local,
            };
        }
        // open_id is handled by the eframe window visibility
    }
    false
}
```

- [ ] **Step 3: Wire tray into main.rs**

The tray needs to be created on the main thread. Add to main.rs before `eframe::run_native`:

```rust
mod tray;

// In main(), before eframe::run_native:
let _tray = tray::create_tray();
```

Note: Full tray integration (polling events, quit handling) requires hooking into eframe's update loop. For now, the tray icon appears. Event polling will be added in the DeskserverApp::update method.

- [ ] **Step 4: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p deskserver`
Expected: Compiles. Tray icon appears when app runs.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/tray.rs crates/app/Cargo.toml crates/app/src/main.rs
git commit -m "feat: add system tray icon with menu"
```

---

### Task 5: Add mDNS Discovery

**Files:**
- Create: `crates/app/src/discovery.rs`
- Modify: `crates/app/Cargo.toml` (add mdns-sd)
- Modify: `crates/app/src/main.rs` (spawn discovery thread)
- Modify: `crates/app/src/state.rs` (add discovery flag)
- Modify: `crates/app/src/ui/connections.rs` (wire scan button)

- [ ] **Step 1: Add dependency**

Add to `crates/app/Cargo.toml`:

```toml
mdns-sd = "0.11"
```

- [ ] **Step 2: Create `crates/app/src/discovery.rs`**

```rust
use crate::state::{AppState, LogLevel, Role, ServerInfo};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SERVICE_TYPE: &str = "_deskserver._tcp.local.";

pub fn advertise_server(state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        let mdns = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                state.lock().unwrap().log(LogLevel::Warning, &format!("mDNS init failed: {}", e));
                return;
            }
        };

        let (name, port) = {
            let s = state.lock().unwrap();
            (s.machine_name.clone(), s.port)
        };

        let host_ip = local_ip().unwrap_or_else(|| "0.0.0.0".to_string());
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &name,
            &format!("{}.", hostname::get().unwrap_or_default().to_string_lossy()),
            &host_ip,
            port,
            None,
        );

        match service {
            Ok(svc) => {
                if let Err(e) = mdns.register(svc) {
                    state.lock().unwrap().log(LogLevel::Warning, &format!("mDNS register failed: {}", e));
                } else {
                    state.lock().unwrap().log(LogLevel::Info, "mDNS: advertising server on LAN");
                }
            }
            Err(e) => {
                state.lock().unwrap().log(LogLevel::Warning, &format!("mDNS service error: {}", e));
            }
        }

        // Keep daemon alive
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    });
}

pub fn scan_for_servers(state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        let mdns = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                state.lock().unwrap().log(LogLevel::Warning, &format!("mDNS scan failed: {}", e));
                return;
            }
        };

        let receiver = match mdns.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(e) => {
                state.lock().unwrap().log(LogLevel::Warning, &format!("mDNS browse failed: {}", e));
                return;
            }
        };

        state.lock().unwrap().log(LogLevel::Info, "Scanning LAN for servers...");

        // Listen for 3 seconds
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let addr = info.get_addresses().iter().next()
                        .map(|a| format!("{}:{}", a, info.get_port()))
                        .unwrap_or_default();
                    let name = info.get_fullname().split('.').next()
                        .unwrap_or("Unknown").to_string();

                    let mut s = state.lock().unwrap();
                    if !s.available_servers.iter().any(|srv| srv.addr == addr) {
                        s.available_servers.push(ServerInfo {
                            name,
                            addr: addr.clone(),
                            client_count: 0,
                            latency_ms: 0,
                        });
                        s.log(LogLevel::Success, &format!("Found server: {}", addr));
                    }
                }
                _ => {}
            }
        }

        state.lock().unwrap().log(LogLevel::Info, "LAN scan complete");
        mdns.shutdown().ok();
    });
}

fn local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
```

- [ ] **Step 3: Wire scan button in connections.rs**

Update the scan button click handler:

```rust
    if ui.button("↻ Scan LAN").clicked() {
        state.available_servers.clear();
        crate::discovery::scan_for_servers(
            // We need the Arc — pass it differently
        );
    }
```

Note: The scan function needs the `Arc<Mutex<AppState>>`. The UI module only has `&mut AppState`. To solve this, add a `scan_requested: bool` flag to AppState, set it in the UI, and check it in main.rs's update loop.

Add to `AppState`:
```rust
    pub scan_requested: bool,
```
Default: `false`

In connections.rs scan button:
```rust
    if ui.button("↻ Scan LAN").clicked() {
        state.available_servers.clear();
        state.scan_requested = true;
    }
```

In DeskserverApp::update, after releasing the state lock, check and handle:
```rust
    // After dropping state lock in update()
    // This will be handled by the main loop or a background poller
```

- [ ] **Step 4: Add `mod discovery;` to main.rs**

- [ ] **Step 5: Verify it compiles**

Run: `source "$HOME/.cargo/env" && cargo build -p deskserver`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/discovery.rs crates/app/Cargo.toml crates/app/src/main.rs crates/app/src/state.rs crates/app/src/ui/connections.rs
git commit -m "feat: add mDNS discovery for LAN server scanning"
```

---

### Task 6: Dark Theme and Visual Polish

**Files:**
- Modify: `crates/app/src/main.rs` (set dark theme)
- Modify: `crates/app/src/ui/mod.rs` (apply custom visuals)

- [ ] **Step 1: Apply dark theme in main.rs**

In the `eframe::run_native` closure, set up the dark theme:

```rust
    let state_clone = state.clone();
    eframe::run_native(
        "Deskserver",
        options,
        Box::new(move |cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(18, 18, 32);
            visuals.window_fill = egui::Color32::from_rgb(22, 22, 40);
            visuals.extreme_bg_color = egui::Color32::from_rgb(10, 10, 20);
            visuals.faint_bg_color = egui::Color32::from_rgb(25, 25, 45);
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(DeskserverApp::new(state_clone)))
        }),
    )
    .expect("Failed to start Deskserver");
```

- [ ] **Step 2: Verify visual appearance**

Run: `source "$HOME/.cargo/env" && cargo run -p deskserver`
Expected: Dark theme with deep blue/purple tones matching the mockup.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/main.rs
git commit -m "feat: apply dark theme to match mockup design"
```

---

### Task 7: Build, Test, and Verify

**Files:** None — verification task.

- [ ] **Step 1: Run all existing tests**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: All 17 protocol + keymap tests still pass.

- [ ] **Step 2: Build release binary**

Run: `source "$HOME/.cargo/env" && cargo build --release -p deskserver`
Expected: Binary at `target/release/deskserver`.

- [ ] **Step 3: Verify app launches with all features**

Run the app and verify:
- Window opens with dark theme
- Status bar shows "Deskserver | Disconnected | LOCAL | Server"
- Three tabs: Screen Layout, Connections, Settings
- Layout editor shows draggable screen rectangle
- Settings form is editable
- Event log shows "Deskserver started" entry
- System tray icon appears

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve build/test issues from end-to-end verification"
```

---

## Verification Checklist

- [ ] `cargo test` — all protocol and keymap tests pass
- [ ] `cargo build --release -p deskserver` — app binary compiles
- [ ] App launches with dark theme
- [ ] Status bar shows connection status, mode, role
- [ ] Screen Layout tab: screens can be dragged
- [ ] Connections tab: server view shows client list, client view shows server list + manual connect
- [ ] Settings tab: machine name, role, port, hotkey, checkboxes all editable
- [ ] Event log: shows entries, can collapse/expand, can clear
- [ ] System tray: icon appears with right-click menu
- [ ] Old CLI binaries still compile (`cargo build -p kvm-server -p kvm-client`)
