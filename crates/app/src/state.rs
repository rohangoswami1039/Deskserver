use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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
    pub dragging_screen: Option<usize>,
    pub drag_offset: (f32, f32),
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
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 200.0,
                is_server: true,
            }],
            event_log: VecDeque::new(),
            port: 24800,
            auto_start: false,
            auto_connect: false,
            active_tab: Tab::ScreenLayout,
            log_collapsed: false,
            manual_connect_ip: String::new(),
            dragging_screen: None,
            drag_offset: (0.0, 0.0),
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
