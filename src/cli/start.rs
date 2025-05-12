use macro_deck_driver::MacroDeck;
use regex::Regex;
use std::{
    fs,
    io::{BufRead, BufReader},
    net::TcpListener,
};

use crate::cli::{
    flash::flash_device,
    models::{Config, Message},
};

#[cfg(not(unix))]
fn auto_detect_port() -> Option<String> {
    None
}

#[cfg(unix)]
fn auto_detect_port() -> Option<String> {
    use serialport::available_ports;

    use crate::cli::list::format_ports_name;

    let ports = available_ports().ok()?;
    let ports = format_ports_name(ports);
    let filtered_ports: Vec<String> = ports
        .into_iter()
        .filter(|port| port != "debug-console" && port != "Bluetooth-Incoming-Port")
        .collect();

    if filtered_ports.len() != 1 {
        None
    } else {
        Some(filtered_ports[0].clone())
    }
}

pub fn start(port: Option<String>, config_path: Option<String>, tcp_port: Option<String>) {
    let port = port.or_else(|| {
        let auto_detected_port = auto_detect_port();
        if auto_detected_port.is_none() {
            eprintln!("No serial port specified and no auto-detected port available.");
        }
        auto_detected_port
    });

    if port.is_none() {
        return;
    }

    let port = port.unwrap();
    let re = Regex::new(r"/dev/(cu|tty)\.?(.*)").unwrap();

    #[cfg(unix)]
    let port = if !re.is_match(&port) {
        // TODO not tested
        #[cfg(target_os = "linux")]
        {
            format!("/dev/TTY{}", port)
        }

        #[cfg(target_os = "macos")]
        {
            format!("/dev/cu.{}", port)
        }
    } else {
        port
    };

    println!("Loading configuration...");
    let config_content = match fs::read_to_string(config_path.unwrap_or("config.json".to_string()))
    {
        Ok(content) => content,
        Err(_) => {
            eprintln!("Failed to read config.json");
            return;
        }
    };
    let config: Config = match serde_json::from_str(&config_content) {
        Ok(config) => config,
        Err(_) => {
            eprintln!("Failed to parse config.json");
            return;
        }
    };

    let deck = MacroDeck::new(&port);
    if deck.is_err() {
        eprintln!("Failed to connect to the serial port: {}", port);
        return;
    }
    let deck = deck.unwrap();

    if let Some(status) = config.status.clone() {
        if let Some(command) = status.command {
            println!("Registering status handler...");

            deck.register_status_handler(move |x: u32| {
                let output = std::process::Command::new(command.clone())
                    .args(status.args.clone().unwrap_or_default())
                    .arg(x.to_string())
                    .output();

                if output.is_err() {
                    eprintln!("Failed to execute command: {}", command);
                } else {
                    let output = output.unwrap();
                    if !output.stdout.is_empty() {
                        println!(
                            "Status command output: {}",
                            String::from_utf8_lossy(&output.stdout)
                        );
                    }
                }
            });
        }
    }

    if let Some(buttons) = config.buttons.clone() {
        println!("Registering button handlers...");
        for (key, button) in buttons.iter() {
            let command = button.command.clone();
            if command.is_none() {
                continue;
            }

            let key = key.clone();
            let command = command.unwrap();
            let args = button.args.clone().unwrap_or_default();

            deck.register_handler(&key.clone(), move || {
                let output = std::process::Command::new(command.clone())
                    .args(args.clone())
                    .output();

                if output.is_err() {
                    eprintln!("[{}] Failed to execute command: {}", key, command);
                } else {
                    let output = output.unwrap();
                    println!(
                        "[{}] Command output: {}",
                        key,
                        if output.stdout.is_empty() {
                            "None".to_string()
                        } else {
                            String::from_utf8_lossy(&output.stdout).to_string()
                        }
                    );
                }
            });
        }
    }

    println!("Start listening to the serial port: {}", port);
    deck.start();

    // TCP server
    let tcp_port = tcp_port.unwrap_or("8964".to_string());
    let listener = match TcpListener::bind(format!("127.0.0.1:{}", tcp_port)) {
        Ok(listener) => listener,
        Err(_) => {
            eprintln!("Failed to bind to TCP port");
            return;
        }
    };

    println!("Start listening to the TCP port: {}", tcp_port);
    println!("Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
                continue;
            }
        };
        println!("New connection: {}", stream.peer_addr().unwrap());

        let reader = BufReader::new(stream);
        let mut stop_flag = false;
        for line in reader.lines() {
            let json_str = match line {
                Ok(line) => line,
                Err(e) => {
                    eprintln!("Error reading from stream: {}", e);
                    continue;
                }
            };

            let msg: Message = match serde_json::from_str(&json_str) {
                Ok(message) => message,
                Err(e) => {
                    eprintln!("Failed to parse JSON: {}", e);
                    continue;
                }
            };

            println!("Received Command: {:?}", msg.type_);
            match msg.type_.as_str() {
                "stop" => {
                    println!("Stopping the server...");
                    stop_flag = true;
                    break;
                }
                "flash" => {
                    println!("Flashing the device...");

                    let config = if let Some(config_path) = msg.value {
                        let config_content = match fs::read_to_string(config_path) {
                            Ok(content) => content,
                            Err(_) => {
                                eprintln!("Failed to read config.json");
                                continue;
                            }
                        };

                        match serde_json::from_str(&config_content) {
                            Ok(config) => config,
                            Err(_) => {
                                eprintln!("Failed to parse config.json");
                                continue;
                            }
                        }
                    } else {
                        config.clone()
                    };

                    flash_device(&deck, &config);

                    break;
                }
                _ => {
                    eprintln!("Unknown command: {}", msg.type_);
                }
            }
        }

        if stop_flag {
            break;
        }
    }
}
