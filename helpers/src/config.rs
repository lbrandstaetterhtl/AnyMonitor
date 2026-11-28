use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub runtime_log_file: PathBuf,
    pub monitor_log_dir: PathBuf,
    pub address: Ipv4Addr,
    pub port: u16,
    pub api_key: String
}

impl Config {
    fn system_config_path() -> PathBuf {
        if cfg!(windows) {
            let base = std::env::var("PROGRAMDATA").unwrap_or_else(|_| r"C:\ProgramData".into());
            PathBuf::from(base).join("AnyMonitor").join("config.json")
        } else {
            PathBuf::from("/etc/anymonitor/config.json")
        }
    }
    
    pub fn save(&self) -> Result<(), String> {
        let config_path = Self::system_config_path();
        
        let str_content = serde_json::to_string(self).map_err(|e| e.to_string())?;
        
        fs::write(config_path, str_content).map_err(|e| e.to_string())
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::system_config_path();

        if let Some(dir) = path.parent() {
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
        }

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            
            return Ok(config)
        }

        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn default() -> Self {
        let config_path = Self::system_config_path();
        let runtime_log_file = config_path.parent().unwrap().join("runtime.log");
        let monitor_log_dir = config_path.parent().unwrap().join("monitors");

        Config {runtime_log_file, monitor_log_dir, address: Ipv4Addr::new(127, 0, 0, 1), port: 8000, api_key: String::from("")}
    }
}