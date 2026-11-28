pub mod router;
mod app_state;
pub mod requests;
pub mod errors;
pub mod monitor;

use crate::app_state::AppState;
use helpers::logger::LoggerMode;
use helpers::{config::Config, logger::Logger};
use monitor::MonitorEntry;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::System;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().expect("Couldn't load config, server didn't start");

    let config_arc = Arc::new(config.clone());
    let logger_arc = Arc::new(Logger::new(config.runtime_log_file, LoggerMode::Info, config.monitor_log_dir));
    let monitor_entries: Arc<Mutex<HashMap<Uuid, MonitorEntry>>> = Arc::new(Mutex::new(HashMap::new()));
    let running_loops = Arc::new(Mutex::new(Vec::new()));
    let shutdown = CancellationToken::new();
    let state = AppState {
        logger: logger_arc,
        config: config_arc,
        started_at: Instant::now(),
        system: Arc::new(Mutex::new(System::new()?)),
        monitor_entries,
        shutdown,
        running_loops
    };

    if config.api_key.is_empty() {
        state.logger.set_mode(LoggerMode::Warn);
        let _ = state.logger.runtime_log("api key is empty in config");
    }

    let router = router::get_router()
        .with_state(state.clone());

    let socket_addr = SocketAddr::new(IpAddr::V4(config.address), config.port);
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    println!("Listening on: {}", socket_addr);
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.runtime_log("Server started successfully!");
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown_signal(state.clone())).await?;

    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.runtime_log("Server is shut down!");
    state.shutdown.cancel();
    
    Ok(())
}

async fn shutdown_signal(state: AppState) {

    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Strg+C-Handler couldn't register");
    };

    #[cfg(windows)]
    let terminate = async {
        tokio::signal::windows::ctrl_close()
            .expect("Close-Handler couldn't register")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM-Handler couldn't register")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    let _ = state.logger.runtime_log(format!("shutdown signal received, stopping running loops | count: {}", state.running_loops.lock().unwrap().len()).as_str());
    state.shutdown.cancel();
}
