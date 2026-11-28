use std::net::{IpAddr};
use reqwest::StatusCode;
use uuid::Uuid;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Errors {
    #[error("entry with id {id} not found")]
    EntryNotFound { id: Uuid },
    #[error("loop with id {id} not found")]
    LoopNotFound { id: Uuid },
    #[error("Blocked unauthorized request to {path} from {ip} | no api key was given")]
    NoApiKey { ip: IpAddr , path: String},
    #[error("Blocked unauthorized request to {path} from {ip} | invalid api key was given")]
    InvalidApiKey { ip: IpAddr , path: String},
    #[error("request data was not processable")]
    InvalidDataGiven,
    #[error("monitor with id {id} is already paused")]
    AlreadyPaused { id: Uuid },
    #[error("monitor with id {id} is not paused")]
    NotPaused { id: Uuid },
}

impl Errors {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Errors::EntryNotFound {id: _} => StatusCode::NOT_FOUND,
            Errors::LoopNotFound {id: _} => StatusCode::NOT_FOUND,
            Errors::NoApiKey {ip: _, path: _} => StatusCode::UNAUTHORIZED,
            Errors::InvalidApiKey {ip: _, path: _} => StatusCode::UNAUTHORIZED,
            Errors::InvalidDataGiven => StatusCode::BAD_REQUEST,
            Errors::AlreadyPaused {id: _} => StatusCode::CONFLICT,
            Errors::NotPaused {id: _} => StatusCode::CONFLICT,
        }
    }
}

use helpers::logger::{Logger, LoggerMode};

pub trait LogAndReturn<T> {
    fn log_and_return(self, logger: &Logger) -> Result<T, StatusCode>;
}

impl<T> LogAndReturn<T> for Result<T, Errors> {
    fn log_and_return(self, logger: &Logger) -> Result<T, StatusCode> {
        self.map_err(|e| {
            logger.set_mode(LoggerMode::Warn);
            logger.runtime_log(&e.to_string());
            e.to_status_code()
        })
    }
}