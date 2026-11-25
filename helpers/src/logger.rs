use std::fs;
use std::path::{Path, PathBuf};
use chrono::TimeZone;

pub struct Logger {
    pub path: PathBuf,
    pub mode: LoggerMode
}

pub enum LoggerMode {
    Debug,
    Warn,
    Info,
    Error
}

impl LoggerMode {
    pub fn get_value_from_str(mode: &str) -> Result<LoggerMode, String> {
        match mode.to_lowercase().as_str() {
            "debug" => Ok(LoggerMode::Debug),
            "warn" => Ok(LoggerMode::Warn),
            "info" => Ok(LoggerMode::Info),
            "error" => Ok(LoggerMode::Error),
            _ => Err(format!("couldn't convert string {} to a valid value", mode))
        }
    }
}

impl Logger {
    pub fn get_mode_as_str(&self) -> Result<&str, String> {
        match self.mode {
            LoggerMode::Debug => Ok("DEBUG"),
            LoggerMode::Warn => Ok("WARN"),
            LoggerMode::Info => Ok("INFO"),
            LoggerMode::Error => Ok("ERROR"),
        }
    }

    pub fn log(&self, message: &str) -> Result<(), String> {
        let file_path = &self.path;
        let mode_str = self.get_mode_as_str()?;
        let date = chrono::Local::now();
        let date_formatted = date.format("%Y-%m-%d %H:%M:%S");

        let message = format!("{} | [{}] {}", date_formatted, mode_str, message);

        if let Some(dir) = file_path.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
        }

        fs::write(file_path, message).map_err(|e| e.to_string())
    }

    pub fn new(path: PathBuf, mode: LoggerMode) -> Self{
        Logger {path, mode}
    }

    pub fn set_mode(&mut self, mode: LoggerMode) {
        self.mode = mode;
    }
}