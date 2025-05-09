use serialport::{available_ports, SerialPortInfo};

#[cfg(not(unix))]
pub fn format_ports_name(ports: Vec<SerialPortInfo>) -> Vec<String> {
    ports.iter().map(|port| port.port_name.clone()).collect()
}

#[cfg(unix)]
pub fn format_ports_name(ports: Vec<SerialPortInfo>) -> Vec<String> {
    use regex::Regex;
    let re = Regex::new(r"/dev/(cu|tty)\.?(.*)").unwrap();
    let mut unique_names = std::collections::HashSet::new();

    ports
        .iter()
        .map(|port| {
            re.captures(&port.port_name)
                .and_then(|caps| caps.get(2).map(|m| m.as_str().to_string()))
                .unwrap_or_else(|| port.port_name.clone())
        })
        .filter(|name| unique_names.insert(name.clone()))
        .collect()
}

pub fn list() {
    match available_ports() {
        Ok(ports) => {
            if ports.is_empty() {
                println!("No serial ports found.");
            } else {
                println!("Available Serial Ports:");
                for port in format_ports_name(ports) {
                    println!("- {}", port);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to list serial ports: {}", e);
        }
    }
}
