use std::fs;
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::io::Write;
use chrono::TimeZone;

pub struct Logger {
    pub path: PathBuf,
    pub mode: RwLock<LoggerMode>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        match *self.mode.write().unwrap() {
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

        if !file_path.exists() {
            File::create(file_path).map_err(|e| e.to_string())?;
        }

        let mut file = OpenOptions::new().write(true).append(true).open(file_path).map_err(|e| e.to_string())?;

        writeln!(file, "{message}").map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn new(path: PathBuf, mode: LoggerMode) -> Self{
        Logger {path, mode: RwLock::new(mode)}
    }

    pub fn set_mode(&self, mode: LoggerMode) {
        *self.mode.write().unwrap() = mode;
    }
}