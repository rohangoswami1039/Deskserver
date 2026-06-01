use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    Server,
    Client,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputMode {
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub ip: String,
    pub resolution: String,
    pub latency_ms: f32,
    pub connected_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub addr: String,
    pub client_count: u32,
    pub latency_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub is_server: bool,
    pub real_width: u32,
    pub real_height: u32,
}

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
    pub overlap_start: f32,
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

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Mode,
}

pub enum NetworkCommand {
    StartServer { port: u16 },
    ConnectTo { addr: String },
    Disconnect,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    ScreenLayout,
    Connections,
    Settings,
}

pub struct AppState {
    pub machine_name: String,
    pub role: Role,
    pub mode: InputMode,
    pub connected_clients: Vec<ClientInfo>,
    pub available_servers: Vec<ServerInfo>,
    pub connected_server: Option<ServerInfo>,
    pub screens: Vec<ScreenConfig>,
    pub event_log: VecDeque<LogEntry>,
    pub port: u16,
    pub auto_start: bool,
    pub auto_connect: bool,
    pub active_tab: Tab,
    pub log_collapsed: bool,
    pub manual_connect_ip: String,
    pub network_tx: Option<mpsc::Sender<NetworkCommand>>,
    pub dragging_screen: Option<usize>,
    pub drag_offset: (f32, f32),
    pub scan_requested: bool,
    pub edge_links: Vec<EdgeLink>,
    pub virtual_cursor: Option<VirtualCursor>,
}

impl Default for AppState {
    fn default() -> Self {
        let machine_name = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            machine_name: machine_name.clone(),
            role: Role::Server,
            mode: InputMode::Local,
            connected_clients: Vec::new(),
            available_servers: Vec::new(),
            connected_server: None,
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
            event_log: VecDeque::new(),
            port: 24800,
            auto_start: false,
            auto_connect: false,
            active_tab: Tab::ScreenLayout,
            log_collapsed: false,
            manual_connect_ip: String::new(),
            network_tx: None,
            dragging_screen: None,
            drag_offset: (0.0, 0.0),
            scan_requested: false,
            edge_links: Vec::new(),
            virtual_cursor: None,
        }
    }
}

impl AppState {
    pub fn log(&mut self, message: impl Into<String>, level: LogLevel) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.event_log.push_back(LogEntry {
            timestamp,
            message: message.into(),
            level,
        });
        if self.event_log.len() > 500 {
            self.event_log.pop_front();
        }
    }
}
