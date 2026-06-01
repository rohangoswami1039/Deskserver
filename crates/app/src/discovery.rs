use crate::state::{AppState, LogLevel, ServerInfo};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SERVICE_TYPE: &str = "_deskserver._tcp.local.";

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

pub fn advertise_server(state: Arc<Mutex<AppState>>) {
    std::thread::spawn(move || {
        let (machine_name, port) = {
            let s = state.lock().unwrap();
            (s.machine_name.clone(), s.port)
        };

        let daemon = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                state.lock().unwrap().log(
                    format!("mDNS advertise failed to create daemon: {}", e),
                    LogLevel::Warning,
                );
                return;
            }
        };

        let ip = get_local_ip().unwrap_or_else(|| "0.0.0.0".to_string());

        // instance name must be unique; use machine name
        let instance_name = machine_name.replace(' ', "-");
        let host_name = format!("{}.local.", instance_name);

        let properties: HashMap<String, String> = HashMap::new();

        let service_info = match ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            ip.as_str(),
            port,
            Some(properties),
        ) {
            Ok(info) => info,
            Err(e) => {
                state.lock().unwrap().log(
                    format!("mDNS failed to create service info: {}", e),
                    LogLevel::Warning,
                );
                return;
            }
        };

        match daemon.register(service_info) {
            Ok(_) => {
                state.lock().unwrap().log(
                    format!("mDNS advertising as '{}' on {}:{}", instance_name, ip, port),
                    LogLevel::Success,
                );
            }
            Err(e) => {
                state.lock().unwrap().log(
                    format!("mDNS register failed: {}", e),
                    LogLevel::Warning,
                );
                return;
            }
        }

        // Keep the daemon alive
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

pub fn scan_for_servers(state: Arc<Mutex<AppState>>) {
    std::thread::spawn(move || {
        state
            .lock()
            .unwrap()
            .log("Scanning LAN for Deskserver instances...", LogLevel::Info);

        let daemon = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                state.lock().unwrap().log(
                    format!("mDNS scan failed to create daemon: {}", e),
                    LogLevel::Warning,
                );
                return;
            }
        };

        let receiver = match daemon.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(e) => {
                state.lock().unwrap().log(
                    format!("mDNS browse failed: {}", e),
                    LogLevel::Warning,
                );
                return;
            }
        };

        let deadline = std::time::Instant::now() + Duration::from_secs(3);

        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let name = info.get_fullname().to_string();
                    let port = info.get_port();

                    for addr in info.get_addresses() {
                        let addr_str = format!("{}:{}", addr, port);

                        let server = ServerInfo {
                            name: name.clone(),
                            addr: addr_str.clone(),
                            client_count: 0,
                            latency_ms: 0.0,
                        };

                        let mut s = state.lock().unwrap();
                        // Dedup by addr
                        if !s.available_servers.iter().any(|srv| srv.addr == addr_str) {
                            s.log(
                                format!("mDNS found server: {} at {}", name, addr_str),
                                LogLevel::Success,
                            );
                            s.available_servers.push(server);
                        }
                    }
                }
                Ok(_) => {} // other events ignored
                Err(_) => break, // timeout or channel closed
            }
        }

        let found = state.lock().unwrap().available_servers.len();
        state.lock().unwrap().log(
            format!("LAN scan complete. Found {} server(s).", found),
            LogLevel::Info,
        );

        let _ = daemon.shutdown();
    });
}
