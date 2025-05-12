use base64::{engine::general_purpose, Engine as _};
use std::{fs, path::PathBuf};

use crate::cli::models::Config;

use super::models::ButtonConfig;

pub fn get_all_icon_paths(from: &str, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.file_name().unwrap().to_str().unwrap().starts_with(".") {
            continue;
        }

        if path.is_file() {
            paths.push(path);
        } else {
            get_all_icon_paths(path.to_str().unwrap(), paths);
        }
    }
}

pub fn write_icons_to_config(from: String, to: Option<String>) {
    let config_content = match fs::read_to_string(to.clone().unwrap_or("config.json".to_string())) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("Failed to read config.json");
            return;
        }
    };

    let mut config: Config = match serde_json::from_str(&config_content) {
        Ok(config) => config,
        Err(_) => {
            eprintln!("Failed to parse config.json");
            return;
        }
    };
    let mut buttons = config.buttons.clone().unwrap_or_default();

    // write icons to config
    let mut icons = vec![];
    get_all_icon_paths(&from, &mut icons);

    for icon in icons {
        let stripped_path = icon.strip_prefix(&from).unwrap_or(&icon).with_extension("");
        let icon_name = format!("/{}", stripped_path.display());

        let icon_data = match fs::read(&icon) {
            Ok(data) => data,
            Err(_) => {
                eprintln!("Failed to read icon file: {}", icon.display());
                continue;
            }
        };
        let encoded = general_purpose::STANDARD.encode(&icon_data);

        if let Some(button_config) = buttons.get_mut(&icon_name) {
            button_config.icon = Some(encoded);
        } else {
            buttons.insert(
                icon_name,
                ButtonConfig {
                    command: None,
                    args: None,
                    icon: Some(encoded),
                },
            );
        }
    }

    config.buttons = Some(buttons);

    // save config
    let config_json = match serde_json::to_string_pretty(&config) {
        Ok(json) => json,
        Err(_) => {
            eprintln!("Failed to serialize config.json");
            return;
        }
    };

    if let Err(_) = fs::write(to.unwrap_or("config.json".to_string()), config_json) {
        eprintln!("Failed to write config.json");
    } else {
        println!("Icons written to config.json");
    }
}
