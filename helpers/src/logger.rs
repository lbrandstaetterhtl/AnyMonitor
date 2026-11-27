use std::fs;
use std::fs::{OpenOptions};
use std::path::{PathBuf};
use std::sync::RwLock;
use std::io::Write;
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
    pub fn get_mode_as_str(&self) -> &str {
        match *self.mode.write().unwrap() {
            LoggerMode::Debug => "DEBUG",
            LoggerMode::Warn => "WARN",
            LoggerMode::Info => "INFO",
            LoggerMode::Error => "ERROR",
        }
    }

    pub fn log(&self, message: &str) {
        if let Err(e) = self.write_line(message) {
            eprintln!("logger: could not write to {}: {e}", self.path.display());
        }
    }

    fn write_line(&self, message: &str) -> std::io::Result<()> {
        let date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("{date} | [{}] {message}", self.get_mode_as_str());

        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        writeln!(file, "{line}")
    }

    pub fn new(path: PathBuf, mode: LoggerMode) -> Self{
        Logger {path, mode: RwLock::new(mode)}
    }

    pub fn set_mode(&self, mode: LoggerMode) {
        *self.mode.write().unwrap() = mode;
    }
}