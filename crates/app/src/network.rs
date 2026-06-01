use crate::state::{AppState, InputMode, LogLevel, NetworkCommand};
use deskserver_common::{read_msg, InputMsg};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

pub fn spawn_network_thread(state: Arc<Mutex<AppState>>) -> mpsc::Sender<NetworkCommand> {
    let (tx, rx) = mpsc::channel::<NetworkCommand>();

    std::thread::spawn(move || {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                NetworkCommand::StartServer { port } => {
                    handle_start_server(port, state.clone());
                }
                NetworkCommand::ConnectTo { addr } => {
                    handle_connect_to(addr, state.clone());
                }
                NetworkCommand::Disconnect => {
                    state
                        .lock()
                        .unwrap()
                        .log("Disconnected", LogLevel::Warning);
                }
            }
        }
    });

    tx
}

fn handle_start_server(port: u16, state: Arc<Mutex<AppState>>) {
    let bind_addr = format!("0.0.0.0:{}", port);
    state
        .lock()
        .unwrap()
        .log(format!("Starting server on {}", bind_addr), LogLevel::Info);

    let listener = match TcpListener::bind(&bind_addr) {
        Ok(l) => l,
        Err(e) => {
            state
                .lock()
                .unwrap()
                .log(format!("Failed to bind: {}", e), LogLevel::Warning);
            return;
        }
    };

    state
        .lock()
        .unwrap()
        .log(format!("Server listening on {}", bind_addr), LogLevel::Success);

    // Accept one client
    match listener.accept() {
        Ok((stream, peer_addr)) => {
            state
                .lock()
                .unwrap()
                .log(format!("Client connected: {}", peer_addr), LogLevel::Success);

            // Keep connection alive — read messages in a loop
            handle_server_stream(stream, peer_addr.to_string(), state.clone());
        }
        Err(e) => {
            state
                .lock()
                .unwrap()
                .log(format!("Accept error: {}", e), LogLevel::Warning);
        }
    }
}

fn handle_server_stream(mut stream: TcpStream, peer: String, state: Arc<Mutex<AppState>>) {
    loop {
        match read_msg(&mut stream) {
            Ok(msg) => {
                match msg {
                    InputMsg::ScreenEnter => {
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Remote;
                        s.log(
                            format!("ScreenEnter from {}", peer),
                            LogLevel::Mode,
                        );
                    }
                    InputMsg::ScreenLeave => {
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Local;
                        s.log(
                            format!("ScreenLeave from {}", peer),
                            LogLevel::Mode,
                        );
                    }
                    _ => {}
                }
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                state
                    .lock()
                    .unwrap()
                    .log(format!("Client {} disconnected", peer), LogLevel::Warning);
                break;
            }
            Err(e) => {
                state
                    .lock()
                    .unwrap()
                    .log(format!("Stream error from {}: {}", peer, e), LogLevel::Warning);
                break;
            }
        }
    }
}

fn handle_connect_to(addr: String, state: Arc<Mutex<AppState>>) {
    state
        .lock()
        .unwrap()
        .log(format!("Connecting to {}", addr), LogLevel::Info);

    let stream = match TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
        Duration::from_secs(5),
    ) {
        Ok(s) => s,
        Err(e) => {
            state
                .lock()
                .unwrap()
                .log(format!("Connection failed: {}", e), LogLevel::Warning);
            return;
        }
    };

    state
        .lock()
        .unwrap()
        .log(format!("Connected to {}", addr), LogLevel::Success);

    handle_client_stream(stream, addr, state.clone());
}

fn handle_client_stream(mut stream: TcpStream, server_addr: String, state: Arc<Mutex<AppState>>) {
    loop {
        match read_msg(&mut stream) {
            Ok(msg) => {
                match msg {
                    InputMsg::ScreenEnter => {
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Remote;
                        s.log("ScreenEnter received", LogLevel::Mode);
                    }
                    InputMsg::ScreenLeave => {
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Local;
                        s.log("ScreenLeave received", LogLevel::Mode);
                    }
                    _ => {}
                }
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                state
                    .lock()
                    .unwrap()
                    .log(
                        format!("Server {} disconnected", server_addr),
                        LogLevel::Warning,
                    );
                break;
            }
            Err(e) => {
                state
                    .lock()
                    .unwrap()
                    .log(
                        format!("Stream error from {}: {}", server_addr, e),
                        LogLevel::Warning,
                    );
                break;
            }
        }
    }
}
