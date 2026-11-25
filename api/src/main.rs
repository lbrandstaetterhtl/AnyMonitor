pub mod routes;
mod app_state;

use sysinfo::System;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use axum::{Error, Router};
use axum::routing::{any, get};
use helpers::{logger::Logger, config::Config};
use helpers::logger::LoggerMode;
use crate::app_state::AppState;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().map_err(anyhow::Error::msg)?;
    let config_arc = Arc::new(config.clone());
    let logger_arc = Arc::new(Logger::new(config.log_file, LoggerMode::Info));
    let state = AppState { logger: logger_arc, config: config_arc, started_at: Instant::now(), system: Arc::new(Mutex::new(System::new()?)) };

    let router = Router::new()
        .route("/", get(routes::root))
        .route("/status", get(routes::status))
        .with_state(state);

    let socket_addr = SocketAddr::new(IpAddr::V4(config.address), config.port);
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    println!("Listening on: {}", socket_addr);
    axum::serve(listener, router).await?;

    Ok(())
}


