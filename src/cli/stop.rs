use std::io::Write;
use std::net::TcpStream;

use super::models::Message;

pub fn stop(tcp_port: Option<String>) {
    let tcp_port = tcp_port.unwrap_or("8964".to_string());
    let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", tcp_port)) {
        Ok(stream) => stream,
        Err(_) => {
            eprintln!("Failed to connect to TCP port: {}", tcp_port);
            return;
        }
    };

    let msg = Message {
        type_: "stop".to_string(),
        value: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    if writeln!(stream, "{}", json).is_err() {
        eprintln!("Failed to send message");
        return;
    }

    println!("Sent stop command to TCP port");
}
