use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
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
    static_read_handler: Arc<Mutex<Option<Box<dyn Fn(Message) + Send + 'static>>>>,
    read_handler: Arc<Mutex<Vec<Box<dyn Fn(Message) + Send + 'static>>>>,
    info: Arc<Mutex<Option<DeviceInfo>>>,
    icons: Arc<Mutex<HashMap<String, DynamicImage>>>,
    dirs: Arc<Mutex<Option<Vec<PathBuf>>>>,
    status: Arc<Mutex<Option<DynamicImage>>>,
    handlers: Arc<Mutex<HashMap<String, Box<dyn Fn() + Send + 'static>>>>,
    status_handler: Arc<Mutex<Option<Box<dyn Fn(u32) + Send + 'static>>>>,
}

macro_rules! send_and_check_ok {
    ($self:ident, $msg_type:expr, $data:expr, $err_msg:expr) => {{
        $self.write(&Message::new(
            $msg_type.to_string(),
            vec![$data.to_string()],
        ))?;

        let message = $self.read()?;
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

        let read_handler: Arc<Mutex<Vec<Box<dyn Fn(Message) + Send + 'static>>>> =
            Arc::new(Mutex::new(vec![]));
        let static_read_handler: Arc<Mutex<Option<Box<dyn Fn(Message) + Send + 'static>>>> =
            Arc::new(Mutex::new(None));

        let port_clone = port.try_clone().map_err(|_| "Failed to clone port")?;
        let read_handler_clone = read_handler.clone();
        let static_read_handler_clone = static_read_handler.clone();
        thread::spawn(move || {
            let mut buf_reader = BufReader::new(port_clone);
            let mut line_buffer = String::new();

            loop {
                if buf_reader.read_line(&mut line_buffer).is_err() {
                    continue;
                }

                let mesg = match Message::decode(line_buffer.clone()) {
                    Some(msg) => msg,
                    None => {
                        line_buffer.clear();
                        continue;
                    }
                };
                line_buffer.clear();

                let static_handler = static_read_handler_clone.lock().unwrap();
                if let Some(handler) = static_handler.as_ref() {
                    handler(mesg.clone());
                }

                let mut handlers = read_handler_clone.lock().unwrap();
                for handler in handlers.iter() {
                    handler(mesg.clone());
                }
                handlers.clear();
            }
        });

        Ok(MacroDeck {
            port: Arc::new(Mutex::new(port)),
            read_handler,
            static_read_handler,
            info: Arc::new(Mutex::new(None)),
            icons: Arc::new(Mutex::new(HashMap::new())),
            dirs: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(None)),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            status_handler: Arc::new(Mutex::new(None)),
        })
    }

    fn read(&self) -> Result<Message, &str> {
        let mesg: Arc<(Mutex<Option<Message>>, Condvar)> =
            Arc::new((Mutex::new(None), Condvar::new()));

        {
            let mut read_handler = self
                .read_handler
                .lock()
                .map_err(|_| "Failed to lock read handler")?;

            let mesg_clone = mesg.clone();

            read_handler.push(Box::new(move |rec_mesg| {
                let (lock, cvar) = &*mesg_clone;
                let mut mesg = lock.lock().unwrap();
                *mesg = Some(rec_mesg);
                cvar.notify_all();
            }));
        }

        let (lock, cvar) = &*mesg;
        let mesg = lock.lock().unwrap();
        let mesg = cvar
            .wait_timeout(mesg, Duration::from_secs(MAX_TIMEOUT))
            .map_err(|_| "Failed to wait for message")?;

        mesg.0.clone().ok_or("Failed to read message")
    }

    // FIXME probably not working
    fn read_exact(&self, buf: &mut [u8]) -> Result<(), &str> {
        println!("Reading {} bytes", buf.len());

        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;
        let mut buf_reader = BufReader::new(port.as_mut());

        buf_reader
            .read_exact(buf)
            .map_err(|_| "Failed to read buffer")?;

        Ok(())
    }

    fn write(&self, message: &Message) -> Result<(), &str> {
        self.write_buffer(&message.encode())
    }

    fn write_buffer(&self, buffer: &[u8]) -> Result<(), &str> {
        let mut port = self.port.lock().map_err(|_| "Failed to lock port")?;

        let total_bytes = buffer.len();
        let written = port.write(buffer).map_err(|_| "Failed to write to port")?;

        if written == total_bytes {
            Ok(())
        } else {
            Err("Failed to write all bytes")
        }
    }

    pub fn get_info(&self) -> Result<DeviceInfo, &str> {
        let mut info = self.info.lock().map_err(|_| "Failed to lock info")?;
        if info.is_some() {
            return Ok(info.clone().unwrap());
        }

        self.write(&Message::new("li".to_string(), Vec::new()))?;

        let message = self.read()?;
        if message.message_type != "li" {
            return Err("Failed to get device info");
        }

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

        self.write(&Message::new("ri".to_string(), vec![path.to_string()]))?;

        let message = self.read()?;
        if message.message_type != "rd?" {
            return Err("Failed to get icon");
        }

        let size = message.data[0]
            .parse::<u32>()
            .map_err(|_| "Failed to parse size")?;

        self.write(&Message::new("rd".to_string(), vec![]))?;

        let mut buffer = vec![0; size as usize];
        self.read_exact(&mut buffer)?;

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

        self.write(&Message::new(
            "wi".to_string(),
            vec![icon_path.to_string(), buffer.len().to_string()],
        ))?;

        let message = self.read()?;
        if message.message_type != "rd" {
            return Err("Failed to write icon");
        }

        self.write_buffer(&buffer)?;

        let message = self.read()?;
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

        self.write(&Message::new(
            "ss".to_string(),
            vec![x.to_string(), y.to_string(), buffer.len().to_string()],
        ))?;

        let message = self.read()?;
        if message.message_type != "rd" {
            return Err("Failed to set status");
        }

        self.write_buffer(&buffer)?;

        let message = self.read()?;
        if message.message_type != "ok" {
            return Err("Failed to set status");
        }

        old_status.replace(status);
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

        self.write(&Message::new("ld".to_string(), Vec::new()))?;

        let message = self.read()?;
        if message.message_type != "ld" {
            return Err("Failed to list directory");
        }

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
        let mut static_read_handler = self.static_read_handler.lock().unwrap();

        let handlers = self.handlers.clone();
        let status_handler = self.status_handler.clone();

        static_read_handler.replace(Box::new(move |mesg| {
            if mesg.message_type == "bc" {
                let icon_path = &mesg.data[0];
                let handlers = handlers.lock().unwrap();
                let handler = handlers.get(icon_path);

                if let Some(handler) = handler {
                    handler();
                }
            } else if mesg.message_type == "sc" {
                let x = mesg.data[0].parse::<u32>().unwrap();
                let handler = status_handler.lock().unwrap();
                if let Some(handler) = handler.as_ref() {
                    handler(x);
                }
            }
        }));
    }
}
