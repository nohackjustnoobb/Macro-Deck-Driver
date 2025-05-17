use base64::{engine::general_purpose, Engine as _};
use log::{debug, error, info, warn};
use macro_deck_driver::MacroDeck;
use regex::Regex;
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Write as _},
    net::{TcpListener, TcpStream},
    process::Stdio,
    sync::{Arc, Mutex},
    thread,
};

use crate::cli::{
    flash::flash_device,
    models::{Config, Message},
};

#[cfg(not(any(unix, windows)))]
fn auto_detect_port() -> Option<String> {
    None
}

#[cfg(windows)]
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

fn read_and_parse_config(config_path: &str) -> Option<Config> {
    let config_content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(_) => {
            warn!("Failed to read config.json");
            return None;
        }
    };

    match serde_json::from_str(&config_content) {
        Ok(config) => Some(config),
        Err(_) => {
            warn!("Failed to parse config.json");
            None
        }
    }
}

pub fn start(port: Option<String>, config_path: Option<String>, tcp_port: Option<String>) {
    let port = port.or_else(|| {
        let auto_detected_port = auto_detect_port();
        if auto_detected_port.is_none() {
            error!("No serial port specified and no auto-detected port available.");
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

    info!("Loading configuration...");
    let config = match read_and_parse_config(&config_path.unwrap_or("config.json".to_string())) {
        Some(config) => config,
        None => return,
    };

    let deck = Arc::new(MacroDeck::new(&port).expect("Failed to create MacroDeck instance"));

    info!("Starting status handler...");
    if let Some(status) = config.status.clone() {
        if let Some(command) = status.command {
            thread::spawn(move || loop {
                let _ = std::process::Command::new(command.clone())
                    .args(status.args.clone().unwrap_or_default())
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                    .expect("Failed to start status handler")
                    .wait();
            });
        }
    }

    info!("Registering status handler...");
    let status_handler_tcp_write_stream: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));
    {
        let status_handler_tcp_write_stream = status_handler_tcp_write_stream.clone();
        deck.register_status_handler(move |x: u32| {
            debug!("Status clicked: {}", x);

            let mut stream = status_handler_tcp_write_stream.lock().unwrap();
            if stream.is_none() {
                return;
            }

            let stream = stream.as_mut().unwrap();
            let mesg = Message {
                type_: "statusClicked".to_string(),
                value: Some(json!(x)),
            };

            stream
                .write_all(serde_json::to_string(&mesg).unwrap().as_bytes())
                .expect("Failed to write to stream");
        });
    }

    if let Some(buttons) = config.buttons.clone() {
        info!("Registering button handlers...");
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
                    warn!("[{}] Failed to execute command: {}", key, command);
                } else {
                    let output = output.unwrap();
                    debug!(
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

    info!("Start listening to the serial port: {}", port);
    deck.start();

    // TCP server
    let tcp_port = tcp_port.unwrap_or("8964".to_string());
    let listener =
        TcpListener::bind(format!("127.0.0.1:{}", tcp_port)).expect("Failed to bind TCP port");

    info!("Start listening to the TCP port: {}", tcp_port);

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
                continue;
            }
        };
        debug!("New connection: {}", stream.peer_addr().unwrap());

        let mut stop_flag = false;
        let mut set_status_handler = false;

        let reader = BufReader::new(&stream);
        for line in reader.lines() {
            let json_str = match line {
                Ok(line) => line,
                Err(e) => {
                    warn!("Error reading from stream: {}", e);
                    continue;
                }
            };

            let msg: Message = match serde_json::from_str(&json_str) {
                Ok(message) => message,
                Err(e) => {
                    warn!("Failed to parse JSON: {}", e);
                    continue;
                }
            };

            debug!("Received Command: {:?}", msg.type_);
            match msg.type_.as_str() {
                "setStatusHandler" => {
                    debug!("Setting status handler...");
                    set_status_handler = true;
                    break;
                }
                "stop" => {
                    debug!("Stopping the server...");
                    stop_flag = true;
                    break;
                }
                "flash" => {
                    debug!("Flashing the device...");

                    let config = if let Some(config_path) = msg.value {
                        let config_path = match config_path.as_str() {
                            Some(path) => path,
                            None => {
                                warn!("Invalid config path");
                                continue;
                            }
                        };

                        match read_and_parse_config(config_path) {
                            Some(config) => config,
                            None => continue,
                        }
                    } else {
                        config.clone()
                    };

                    flash_device(&deck, &config);

                    break;
                }
                _ => {
                    warn!("Unknown command: {}", msg.type_);
                }
            }
        }

        if stop_flag {
            break;
        }

        if set_status_handler {
            let info = match deck.get_info() {
                Ok(info) => info,
                Err(e) => {
                    error!("Failed to get device info: {}", e);
                    continue;
                }
            };

            let mesg = Message {
                type_: "setStatusHandler".to_string(),
                value: Some(json!([info.width, info.status_bar_height])),
            };

            stream
                .write_all(serde_json::to_string(&mesg).unwrap().as_bytes())
                .expect("Failed to write to stream");

            // Set stream for the status handler
            let write_stream = stream.try_clone().expect("Failed to clone stream");
            status_handler_tcp_write_stream
                .lock()
                .unwrap()
                .replace(write_stream);

            status_tcp_stream_read_handler(stream, deck.clone());
            debug!("Status handler set");
        }
    }
}

fn status_tcp_stream_read_handler(stream: TcpStream, deck: Arc<MacroDeck>) {
    const MAX_TRIES: u32 = 5;

    thread::spawn(move || loop {
        let reader = BufReader::new(&stream);
        for line in reader.lines() {
            let json_str = match line {
                Ok(line) => line,
                Err(e) => {
                    warn!("Error reading from stream: {}", e);
                    // probably the stream is closed
                    break;
                }
            };

            let msg: Message = match serde_json::from_str(&json_str) {
                Ok(message) => message,
                Err(e) => {
                    warn!("Failed to parse JSON: {}", e);
                    continue;
                }
            };

            debug!("Received Command: {:?}", msg.type_);
            match msg.type_.as_str() {
                "setStatus" => {
                    let encoded = msg.value.clone();
                    if encoded.is_none() {
                        warn!("No status provided");
                        continue;
                    }
                    let encoded = encoded.unwrap();
                    let encoded = match encoded.as_str() {
                        Some(encoded) => encoded,
                        None => {
                            warn!("Invalid status");
                            continue;
                        }
                    };

                    let img = match general_purpose::STANDARD.decode(encoded) {
                        Ok(img) => img,
                        Err(_) => {
                            warn!("Failed to decode image");
                            continue;
                        }
                    };

                    let img = match image::load_from_memory(&img) {
                        Ok(img) => img,
                        Err(_) => {
                            warn!("Failed to load image from memory");
                            continue;
                        }
                    };

                    let mut tries = 0;
                    loop {
                        if deck.set_status(img.clone()).is_ok() {
                            debug!("Status set successfully");
                            break;
                        }

                        debug!("Failed to set status, retrying...");

                        tries += 1;
                        if tries >= MAX_TRIES {
                            error!("Failed to set status after {} tries", MAX_TRIES);
                            break;
                        }
                    }
                }
                _ => {
                    warn!("Unknown command: {}", msg.type_);
                }
            }
        }
    });
}
