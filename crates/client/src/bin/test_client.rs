use deskserver_common::{read_msg, InputMsg};
use std::env;
use std::net::TcpStream;

const PORT: u16 = 24800;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: test_client <server-ip>");
        std::process::exit(1);
    }
    let server_ip = &args[1];
    let addr = format!("{}:{}", server_ip, PORT);

    println!("[CLIENT] Connecting to {}...", addr);

    let mut stream = match TcpStream::connect(&addr) {
        Ok(s) => {
            println!("[CLIENT] TCP connection established!");
            s
        }
        Err(e) => {
            eprintln!("[CLIENT] ERROR: failed to connect: {}", e);
            return;
        }
    };

    match stream.set_nodelay(true) {
        Ok(()) => println!("[CLIENT] TCP_NODELAY set"),
        Err(e) => eprintln!("[CLIENT] WARNING: failed to set TCP_NODELAY: {}", e),
    }

    println!("[CLIENT] Waiting for messages...");
    let mut count = 0;

    loop {
        match read_msg(&mut stream) {
            Ok(msg) => {
                count += 1;
                match &msg {
                    InputMsg::MouseMove { x, y } => {
                        println!("[CLIENT] Received #{}: MouseMove x={}, y={}", count, x, y);
                    }
                    InputMsg::MouseButton { button, pressed } => {
                        println!("[CLIENT] Received #{}: MouseButton {:?} pressed={}", count, button, pressed);
                    }
                    InputMsg::Wheel { dx, dy } => {
                        println!("[CLIENT] Received #{}: Wheel dx={}, dy={}", count, dx, dy);
                    }
                    InputMsg::KeyDown { key, modifiers } => {
                        println!("[CLIENT] Received #{}: KeyDown key={}, modifiers={}", count, key, modifiers);
                    }
                    InputMsg::KeyUp { key, modifiers } => {
                        println!("[CLIENT] Received #{}: KeyUp key={}, modifiers={}", count, key, modifiers);
                    }
                    InputMsg::ScreenEnter { x, y } => {
                        println!("[CLIENT] Received #{}: ScreenEnter at ({:.0}, {:.0})", count, x, y);
                    }
                    InputMsg::ScreenLeave => {
                        println!("[CLIENT] Received #{}: ScreenLeave", count);
                    }
                }
            }
            Err(e) => {
                println!("[CLIENT] Connection ended after {} messages: {}", count, e);
                break;
            }
        }
    }

    println!("[CLIENT] Done. Exiting.");
}
