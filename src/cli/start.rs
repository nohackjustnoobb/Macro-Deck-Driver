use macro_deck_driver::MacroDeck;
use regex::Regex;
use serde::Deserialize;
use std::{collections::HashMap, fs};

#[derive(Deserialize, Clone, Debug)]
struct ButtonConfig {
    command: String,
    args: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct Config {
    buttons: Option<HashMap<String, ButtonConfig>>,
    status: Option<ButtonConfig>,
}

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

pub fn start(port: Option<String>) {
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
    let config_content = fs::read_to_string("config.json").expect("Failed to read config.json");
    let config: Config =
        serde_json::from_str(&config_content).expect("Failed to parse config.json");

    let deck = MacroDeck::new(&port);
    if deck.is_err() {
        eprintln!("Failed to connect to the serial port: {}", port);
        return;
    }
    let deck = deck.unwrap();

    if let Some(status) = config.status.clone() {
        println!("Registering status handler...");
        deck.register_status_handler(move |x: u32| {
            let output = std::process::Command::new(status.command.clone())
                .args(status.args.clone())
                .arg(x.to_string())
                .output();

            if output.is_err() {
                eprintln!("Failed to execute command: {}", status.command);
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

    if let Some(buttons) = config.buttons.clone() {
        println!("Registering button handlers...");
        for (key, button) in buttons.iter() {
            let command = button.command.clone();
            let args = button.args.clone();
            deck.register_handler(key, move || {
                let output = std::process::Command::new(command.clone())
                    .args(args.clone())
                    .output();

                if output.is_err() {
                    eprintln!("Failed to execute command: {}", command);
                } else {
                    let output = output.unwrap();
                    if !output.stdout.is_empty() {
                        println!(
                            "Button command output: {}",
                            String::from_utf8_lossy(&output.stdout)
                        );
                    }
                }
            });
        }
    }

    println!("Starting listening to the serial port: {}", port);
    println!("Press Ctrl+C to stop.");
    deck.start();
}
