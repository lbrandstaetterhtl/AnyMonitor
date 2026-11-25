use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub log_file: PathBuf,
    pub address: String,
    pub port: u16,
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
        let log_file = config_path.parent().unwrap().join("runtime.log");

        Config {log_file, address: "127.0.0.1".to_string(), port: 8000}
    }
}