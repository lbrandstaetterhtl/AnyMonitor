use std::fs;
use std::fs::{OpenOptions};
use std::path::{PathBuf};
use std::sync::RwLock;
use std::io::Write;
use uuid::Uuid;

pub struct Logger {
    pub path: PathBuf,
    pub mode: RwLock<LoggerMode>,
    pub monitor_log_dir: PathBuf,
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

    pub fn runtime_log(&self, message: &str) {
        if let Err(e) = self.write_line(message, None) {
            eprintln!("logger: could not write to {}: {e}", self.path.display());
        }
    }

    pub fn monitor_log(&self, message: &str, log_file: String) {
        let file = self.monitor_log_dir.join(log_file);

        if !file.parent().unwrap().exists() {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
        }

        if let Err(e) = self.write_line(message, Some(file.clone())) {
            eprintln!("logger: could not write to {}: {e}", file.display());
        }
    }

    fn write_line(&self, message: &str, log_file: Option<PathBuf>) -> std::io::Result<()> {
        let date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("{date} | [{}] {message}", self.get_mode_as_str());

        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }

        let target = log_file.unwrap_or_else(|| self.path.clone());

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)?;

        writeln!(file, "{}", line)

    }

    pub fn new(path: PathBuf, mode: LoggerMode, monitor_log_dir: PathBuf) -> Self{
        Logger {path, mode: RwLock::new(mode), monitor_log_dir}
    }

    pub fn set_mode(&self, mode: LoggerMode) {
        *self.mode.write().unwrap() = mode;
    }
}