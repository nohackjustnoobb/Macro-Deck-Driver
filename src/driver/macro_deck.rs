use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::{spawn, JoinHandle},
};

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};
use serialport::SerialPort;

use super::message::Message;

const MAX_TIMEOUT: u64 = 3;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub width: u32,
    pub height: u32,
    pub buttons_per_row: u32,
    pub num_of_rows: u32,
    pub gap_size: u32,
    pub button_size: u32,
    pub status_bar_height: u32,
}

pub struct MacroDeck {
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    info: Arc<Mutex<Option<DeviceInfo>>>,
    icons: Arc<Mutex<HashMap<String, DynamicImage>>>,
    dirs: Arc<Mutex<Option<Vec<PathBuf>>>>,
    status: Arc<Mutex<Option<DynamicImage>>>,
    handoff: Arc<(Mutex<bool>, Condvar)>,
    handlers: Arc<Mutex<HashMap<String, Box<dyn Fn() + Send + 'static>>>>,
    status_handler: Arc<Mutex<Option<Box<dyn Fn(u32) + Send + 'static>>>>,
    running: Arc<Mutex<bool>>,
    thread_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

macro_rules! send_and_check_ok {
    ($self:ident, $msg_type:expr, $data:expr, $err_msg:expr) => {{
        let (lock, cvar) = &*$self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = $self.port.lock().map_err(|_| "Failed to lock port")?;

        port.write(&Message::new($msg_type.to_string(), vec![$data.to_string()]).encode())
            .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        drop(port);
        cvar.notify_all();

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type != "ok" {
            return Err($err_msg);
        }

        Ok(())
    }};
}

fn find_patch(img1: &DynamicImage, img2: &DynamicImage) -> Option<(u32, u32, DynamicImage)> {
    if img1.dimensions() != img2.dimensions() {
        return None;
    }

    let (width, height) = img1.dimensions();

    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut has_diff = false;

    for y in 0..height {
        for x in 0..width {
            if img1.get_pixel(x, y) != img2.get_pixel(x, y) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                has_diff = true;
            }
        }
    }

    if !has_diff {
        return None;
    }

    let patch_width = max_x - min_x + 1;
    let patch_height = max_y - min_y + 1;

    let patch = img2.crop_imm(min_x, min_y, patch_width, patch_height);

    Some((min_x, min_y, patch))
}

impl MacroDeck {
    pub fn new(path: &str) -> Result<Self, &str> {
        let port = serialport::new(path, 115200)
            .timeout(std::time::Duration::from_secs(MAX_TIMEOUT))
            .open()
            .map_err(|_| "Failed to open port")?;

        Ok(MacroDeck {
            port: Arc::new(Mutex::new(port)),
            info: Arc::new(Mutex::new(None)),
            icons: Arc::new(Mutex::new(HashMap::new())),
            dirs: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(None)),
            handoff: Arc::new((Mutex::new(false), Condvar::new())),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            status_handler: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
        })
    }

    pub fn get_info(&self) -> Result<DeviceInfo, &str> {
        let mut info = self.info.lock().map_err(|_| "Failed to lock info")?;
        if info.is_some() {
            return Ok(info.clone().unwrap());
        }

        let (lock, cvar) = &*self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;
        port.write(&Message::new("li".to_string(), Vec::new()).encode())
            .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        drop(port);
        cvar.notify_all();

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;

        let width = message.data[0].parse().unwrap();
        let height = message.data[1].parse().unwrap();
        let buttons_per_row = message.data[2].parse().unwrap();
        let num_of_rows = message.data[3].parse().unwrap();
        let gap_size = message.data[4].parse().unwrap();
        let button_size = (width - (buttons_per_row - 1) * gap_size) / buttons_per_row;
        let status_bar_height = height - num_of_rows * button_size - num_of_rows * gap_size;

        let new_info = Some(DeviceInfo {
            width,
            height,
            buttons_per_row,
            num_of_rows,
            gap_size,
            button_size,
            status_bar_height,
        });

        *info = new_info.clone();

        Ok(new_info.unwrap())
    }

    pub fn get_icon(&self, path: &str) -> Result<DynamicImage, &str> {
        let mut icons = self.icons.lock().map_err(|_| "Failed to lock icons")?;
        if let Some(icon) = icons.get(path) {
            return Ok(icon.clone());
        }

        let (lock, cvar) = &*self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;
        port.write(&Message::new("ri".to_string(), vec![path.to_string()]).encode())
            .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type == "rd?" {}
        let size = message.data[0]
            .parse::<u32>()
            .map_err(|_| "Failed to parse size")?;

        buf_reader
            .get_mut()
            .write(&Message::new("rd".to_string(), vec![]).encode())
            .map_err(|_| "Failed to write to port")?;

        let mut buffer = vec![0; size as usize];
        buf_reader
            .read_exact(&mut buffer)
            .map_err(|_| "Failed to read icon")?;

        drop(port);
        cvar.notify_all();

        let icon = ImageReader::new(Cursor::new(buffer))
            .with_guessed_format()
            .map_err(|_| "Failed to decode icon")?
            .decode()
            .map_err(|_| "Failed to decode icon")?;

        icons.insert(path.to_string(), icon.clone());

        return Ok(icon);
    }

    fn add_path_to_dirs(&self, path: &str) -> Result<(), &str> {
        let mut dirs = self.dirs.lock().map_err(|_| "Failed to lock dirs")?;
        if dirs.is_some() {
            let mut seen: HashSet<PathBuf> = dirs.as_mut().unwrap().iter().cloned().collect();
            let mut current = Some(Path::new(path));
            let mut to_add = vec![];

            while let Some(p) = current {
                current = p.parent();
                if current.is_none() {
                    break;
                }

                if !seen.contains(p) {
                    to_add.push(p.to_path_buf());
                    seen.insert(p.to_path_buf());
                }
            }

            dirs.as_mut().unwrap().extend(to_add);
        }

        Ok(())
    }

    pub fn set_icon(&self, icon_path: &str, icon: DynamicImage) -> Result<(), &str> {
        // Convert the icon to JPEG format
        let mut buffer = Vec::new();
        icon.write_to(&mut Cursor::new(&mut buffer), ImageFormat::Jpeg)
            .map_err(|_| "Failed to write icon")?;

        // Update cache
        let mut icons = self.icons.lock().map_err(|_| "Failed to lock icons")?;
        icons.insert(icon_path.to_string(), icon);

        let (lock, cvar) = &*self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;

        port.write(
            &Message::new(
                "wi".to_string(),
                vec![icon_path.to_string(), buffer.len().to_string()],
            )
            .encode(),
        )
        .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type != "rd" {
            return Err("Failed to write icon");
        }

        buf_reader
            .get_mut()
            .write(&buffer)
            .map_err(|_| "Failed to write icon")?;

        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        drop(port);
        cvar.notify_all();

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type != "ok" {
            return Err("Failed to write icon");
        }

        // Update dirs
        self.add_path_to_dirs(icon_path)?;

        Ok(())
    }

    pub fn set_status(&self, status: DynamicImage) -> Result<(), &str> {
        if status.dimensions() != (self.get_info()?.width, self.get_info()?.status_bar_height) {
            return Err("Status image size does not match");
        }

        let mut old_status = self.status.lock().map_err(|_| "Failed to lock status")?;
        let (x, y, patch) = if let Some(old_status) = old_status.as_ref() {
            if let Some(result) = find_patch(old_status, &status) {
                result
            } else {
                return Ok(());
            }
        } else {
            (0, 0, status.clone())
        };

        let mut buffer = Vec::new();
        patch
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Jpeg)
            .map_err(|_| "Failed to write image")?;

        let (lock, cvar) = &*self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;

        port.write(
            &Message::new(
                "ss".to_string(),
                vec![x.to_string(), y.to_string(), buffer.len().to_string()],
            )
            .encode(),
        )
        .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type != "rd" {
            return Err("Failed to set status");
        }

        buf_reader
            .get_mut()
            .write(&buffer)
            .map_err(|_| "Failed to set status")?;

        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        drop(port);
        cvar.notify_all();

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        if message.message_type != "ok" {
            return Err("Failed to set status");
        }

        *old_status = Some(status);
        Ok(())
    }

    pub fn get_status(&self) -> Result<DynamicImage, &str> {
        let status = self.status.lock().map_err(|_| "Failed to lock status")?;
        if let Some(status) = status.clone() {
            return Ok(status);
        }

        Err("Status not set")
    }

    pub fn list_directory(&self) -> Result<Vec<PathBuf>, &str> {
        let mut dirs = self.dirs.lock().map_err(|_| "Failed to lock dirs")?;
        if dirs.is_some() {
            return Ok(dirs.clone().unwrap());
        }

        let (lock, cvar) = &*self.handoff;
        let mut handoff = lock.lock().map_err(|_| "Failed to lock handoff")?;
        *handoff = true;
        drop(handoff);

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;
        port.write(&Message::new("ld".to_string(), Vec::new()).encode())
            .map_err(|_| "Failed to write to port")?;

        let mut buf_reader = BufReader::new(port.as_mut());
        let mut line_buffer = String::new();
        buf_reader
            .read_line(&mut line_buffer)
            .map_err(|_| "Failed to read message")?;

        drop(port);
        cvar.notify_all();

        let message = Message::decode(line_buffer).ok_or("Failed to decode message")?;
        let new_dirs: Vec<PathBuf> = message.data.iter().map(|str| PathBuf::from(str)).collect();

        *dirs = Some(new_dirs.clone());

        Ok(new_dirs)
    }

    pub fn set_profile(&self, profile_name: &str) -> Result<(), &str> {
        send_and_check_ok!(self, "sp", profile_name, "Failed to set profile")
    }

    pub fn create_folder(&self, path: &str) -> Result<(), &str> {
        let result = send_and_check_ok!(self, "cf", path, "Failed to create folder");

        if result.is_ok() {
            self.add_path_to_dirs(path)?;
        }

        result
    }

    fn remove_path_from_dirs(&self, path: &str) -> Result<(), &str> {
        let mut dirs = self.dirs.lock().map_err(|_| "Failed to lock dirs")?;
        if dirs.is_some() {
            dirs.as_mut()
                .unwrap()
                .retain(|p| !p.starts_with(&Path::new(path)));
        }

        Ok(())
    }

    pub fn remove_icon(&self, icon_path: &str) -> Result<(), &str> {
        let result = send_and_check_ok!(self, "di", icon_path, "Failed to remove icon");

        if result.is_ok() {
            self.remove_path_from_dirs(icon_path)?;
        }

        result
    }

    pub fn remove_folder(&self, folder_path: &str) -> Result<(), &str> {
        let result = send_and_check_ok!(self, "df", folder_path, "Failed to remove folder");

        if result.is_ok() {
            self.remove_path_from_dirs(folder_path)?;
        }

        result
    }

    pub fn register_handler<F>(&self, button_path: &str, handler: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.insert(button_path.to_string(), Box::new(handler));
    }

    pub fn register_status_handler<F>(&self, handler: F)
    where
        F: Fn(u32) + Send + 'static,
    {
        let mut status_handler = self.status_handler.lock().unwrap();
        *status_handler = Some(Box::new(handler));
    }

    pub fn start(&self) {
        let port_lock = self.port.clone();
        let handoff_lock = self.handoff.clone();

        let handlers = self.handlers.clone();
        let status_handler = self.status_handler.clone();
        let running = self.running.clone();

        {
            let mut is_running = running.lock().unwrap();
            *is_running = true;
        }

        let handle = spawn(move || {
            let mut port = port_lock.lock().unwrap();

            loop {
                // Check if we should exit
                {
                    let is_running = running.lock().unwrap();
                    if !*is_running {
                        break;
                    }
                }

                // Check if we need to hand off the port
                {
                    let (lock, cvar) = &*handoff_lock;
                    let mut lock = match lock.lock() {
                        Ok(lock) => lock,
                        Err(_) => continue,
                    };

                    if *lock {
                        // handoff
                        *lock = false;
                        drop(port);

                        // relock the port
                        drop(cvar.wait(lock).unwrap());
                        port = port_lock.lock().unwrap();
                    }
                }

                let mut buf_reader = BufReader::new(port.as_mut());
                let mut line_buffer = String::new();
                if buf_reader.read_line(&mut line_buffer).is_err() {
                    continue;
                };

                let message = Message::decode(line_buffer);
                if message.is_none() {
                    continue;
                }

                let message = message.unwrap();
                if message.message_type == "bc" {
                    let icon_path = &message.data[0];
                    let handlers = handlers.lock().unwrap();
                    let handler = handlers.get(icon_path);

                    if let Some(handler) = handler {
                        handler();
                    }
                } else if message.message_type == "sc" {
                    let x = message.data[0].parse::<u32>().unwrap();
                    let handler = status_handler.lock().unwrap();
                    if let Some(handler) = handler.as_ref() {
                        handler(x);
                    }
                }
            }
        });

        // Store the thread handle
        if let Ok(mut thread_handle) = self.thread_handle.lock() {
            *thread_handle = Some(handle);
        }
    }
}

impl Drop for MacroDeck {
    fn drop(&mut self) {
        if let Ok(mut is_running) = self.running.lock() {
            *is_running = false;
        }

        if let Ok(mut handle) = self.thread_handle.lock() {
            if let Some(thread) = handle.take() {
                let _ = thread.join();
            }
        }
    }
}
