use crate::state::{AppState, InputMode, LogLevel, NetworkCommand};
use deskserver_common::{read_msg, write_msg, InputMsg, MouseButton};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

#[cfg(target_os = "macos")]
use deskserver_common::keymap::neutral_to_macos_keycode;
#[cfg(target_os = "windows")]
use deskserver_common::keymap::neutral_to_windows_vk;

fn map_button(b: &MouseButton) -> Button {
    match b {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
    }
}

fn neutral_to_local_keycode(neutral_key: u32) -> Option<u16> {
    #[cfg(target_os = "macos")]
    { neutral_to_macos_keycode(neutral_key).map(|k| k as u16) }
    #[cfg(target_os = "windows")]
    { neutral_to_windows_vk(neutral_key).map(|k| k as u16) }
}

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
            if let Err(e) = stream.set_nodelay(true) {
                state.lock().unwrap().log(format!("set_nodelay failed: {}", e), LogLevel::Warning);
            }

            state
                .lock()
                .unwrap()
                .log(format!("Client connected: {}", peer_addr), LogLevel::Success);

            // Send test messages to verify connection
            {
                let mut s = &stream;
                let test_msgs = vec![
                    ("MouseMove(100, 100)", InputMsg::MouseMove { x: 100.0, y: 100.0 }),
                    ("MouseButton(Left, press)", InputMsg::MouseButton { button: MouseButton::Left, pressed: true }),
                    ("MouseButton(Left, release)", InputMsg::MouseButton { button: MouseButton::Left, pressed: false }),
                ];
                for (label, msg) in &test_msgs {
                    match write_msg(&mut s, msg) {
                        Ok(()) => {
                            state.lock().unwrap().log(format!("Test: {} — OK", label), LogLevel::Info);
                        }
                        Err(e) => {
                            state.lock().unwrap().log(format!("Test failed: {} — {}", label, e), LogLevel::Warning);
                            return;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
                state.lock().unwrap().log("Connection verified!", LogLevel::Success);
            }

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
                    InputMsg::ScreenEnter { .. } => {
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
    let full_addr = if addr.contains(':') {
        addr.clone()
    } else {
        format!("{}:24800", addr)
    };

    state
        .lock()
        .unwrap()
        .log(format!("Connecting to {}", full_addr), LogLevel::Info);

    let socket_addr: std::net::SocketAddr = match full_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            state.lock().unwrap().log(
                format!("Invalid address '{}': {}", full_addr, e),
                LogLevel::Warning,
            );
            return;
        }
    };

    let stream = match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
        Ok(s) => {
            s.set_nodelay(true).ok();
            s
        }
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
        .log(format!("Connected to {}", full_addr), LogLevel::Success);

    handle_client_stream(stream, addr, state.clone());
}

fn handle_client_stream(mut stream: TcpStream, server_addr: String, state: Arc<Mutex<AppState>>) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            state.lock().unwrap().log(
                format!("Failed to create Enigo: {}", e),
                LogLevel::Warning,
            );
            return;
        }
    };

    let mut remote_mode = false;

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => {
                match msg {
                    InputMsg::MouseMove { x, y } => {
                        if remote_mode {
                            enigo.move_mouse(x as i32, y as i32, Coordinate::Rel).ok();
                        } else {
                            enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                        }
                    }
                    InputMsg::MouseButton { button, pressed } => {
                        let btn = map_button(&button);
                        let dir = if pressed { Direction::Press } else { Direction::Release };
                        enigo.button(btn, dir).ok();
                    }
                    InputMsg::Wheel { dy, .. } => {
                        enigo.scroll(dy as i32, Axis::Vertical).ok();
                    }
                    InputMsg::KeyDown { key, .. } => {
                        if let Some(kc) = neutral_to_local_keycode(key) {
                            enigo.raw(kc, Direction::Press).ok();
                        }
                    }
                    InputMsg::KeyUp { key, .. } => {
                        if let Some(kc) = neutral_to_local_keycode(key) {
                            enigo.raw(kc, Direction::Release).ok();
                        }
                    }
                    InputMsg::ScreenEnter { x, y } => {
                        remote_mode = true;
                        enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).ok();
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Remote;
                        s.log(
                            format!("ScreenEnter at ({:.0}, {:.0}) — controlling this machine", x, y),
                            LogLevel::Mode,
                        );
                    }
                    InputMsg::ScreenLeave => {
                        remote_mode = false;
                        let mut s = state.lock().unwrap();
                        s.mode = InputMode::Local;
                        s.log("ScreenLeave — control returned to server", LogLevel::Mode);
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                state.lock().unwrap().log(
                    format!("Server {} disconnected", server_addr),
                    LogLevel::Warning,
                );
                break;
            }
            Err(e) => {
                state.lock().unwrap().log(
                    format!("Stream error from {}: {}", server_addr, e),
                    LogLevel::Warning,
                );
                break;
            }
        }
    }
}
