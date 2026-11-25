use sysinfo::System;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use helpers::config::Config;
use helpers::logger::Logger;

#[derive(Clone)]
pub struct AppState {
    pub logger: Arc<Logger>,
    pub config: Arc<Config>,
    pub started_at: Instant,
    pub system: Arc<Mutex<System>>
}