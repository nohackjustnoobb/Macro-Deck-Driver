use std::net::TcpStream;
use std::path::PathBuf;
use std::str::FromStr;
use std::{collections::HashMap, io::Write};

use base64::engine::general_purpose;
use base64::Engine as _;
use image::imageops::FilterType;
use image::{GenericImage, ImageBuffer, Rgb, RgbImage};
use macro_deck_driver::MacroDeck;
use serde_json::json;

use super::models::{Config, Message};

pub fn flash_device(deck: &MacroDeck, config: &Config) {
    println!("Creating aio images...");

    if config.buttons.is_none() {
        eprintln!("No buttons found in config");
        return;
    }

    // Group icons by directory
    let mut dir_icons: HashMap<String, HashMap<usize, String>> = HashMap::new();
    for (key, button) in config.buttons.as_ref().unwrap() {
        if let Some(icon) = &button.icon {
            let path = match PathBuf::from_str(key) {
                Ok(path) => path,
                Err(_) => {
                    eprintln!("Invalid icon path: {}", key);
                    continue;
                }
            };

            let parent = match path.parent() {
                Some(parent) => parent.to_str().unwrap(),
                None => {
                    eprintln!("Invalid icon path: {}", key);
                    continue;
                }
            };

            let idx = match path.file_name() {
                Some(idx) => idx,
                None => {
                    eprintln!("Invalid icon path: {}", key);
                    continue;
                }
            };
            let idx = idx.to_str().unwrap();
            let idx = match idx.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    eprintln!("Invalid icon path: {}", key);
                    continue;
                }
            };

            if let Some(dir) = dir_icons.get_mut(parent) {
                dir.insert(idx, icon.clone());
            } else {
                let mut dir = HashMap::new();
                dir.insert(idx, icon.clone());
                dir_icons.insert(parent.to_string(), dir);
            }
        }
    }

    let info = match deck.get_info() {
        Ok(info) => info,
        Err(_) => {
            eprintln!("Failed to get device info");
            return;
        }
    };
    let width = info.width;
    let height = info.height - info.gap_size - info.status_bar_height;

    // Create a new image for each directory
    let mut aio_hashmap = HashMap::new();
    for (dir, icons) in dir_icons {
        let mut aio: RgbImage = ImageBuffer::from_pixel(width, height, Rgb([0, 0, 0]));

        for (idx, icon) in icons {
            let icon = match general_purpose::STANDARD.decode(icon) {
                Ok(icon) => icon,
                Err(_) => {
                    eprintln!("Failed to decode icon: {}", idx);
                    continue;
                }
            };

            let icon = match image::load_from_memory(&icon) {
                Ok(icon) => icon,
                Err(_) => {
                    eprintln!("Failed to load icon: {}", idx);
                    continue;
                }
            };

            // Resize the icon to fit the button size
            let icon = icon
                .resize_exact(info.button_size, info.button_size, FilterType::Lanczos3)
                .to_rgb8();

            let col = (idx as u32) % info.buttons_per_row;
            let row = (idx as u32) / info.buttons_per_row;

            let x = col * (info.button_size + info.gap_size);
            let y = row * (info.button_size + info.gap_size);

            if aio.copy_from(&icon, x, y).is_err() {
                eprintln!("Failed to copy icon: {}", idx);
                continue;
            }
        }

        aio_hashmap.insert(dir, aio);
    }

    // Format the device
    println!("Formatting device...");
    if deck.remove_folder("/").is_err() {
        eprintln!("Failed to format device");
        return;
    }

    // Write the images to the device
    for (dir, aio) in aio_hashmap {
        let dir = format!("{}/aio.jpg", dir);
        println!("Writing icon: {}", dir);

        if deck
            .set_icon(&dir, image::DynamicImage::ImageRgb8(aio))
            .is_err()
        {
            eprintln!("Failed to write icon: {}", dir);
            continue;
        }
    }

    println!("Flash complete!");
}

pub fn flash(tcp_port: Option<String>, config_path: Option<String>) {
    let tcp_port = tcp_port.unwrap_or("8964".to_string());
    let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", tcp_port)) {
        Ok(stream) => stream,
        Err(_) => {
            eprintln!("Failed to connect to TCP port: {}", tcp_port);
            return;
        }
    };

    let msg = Message {
        type_: "flash".to_string(),
        value: config_path.map(|v| json!(v)),
    };

    let json = serde_json::to_string(&msg).unwrap();
    if writeln!(stream, "{}", json).is_err() {
        eprintln!("Failed to send message");
        return;
    }

    println!("Sent flash command to TCP port");

    // TODO response
}
