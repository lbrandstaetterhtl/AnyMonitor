pub mod routes_handlers;

use axum::Router;
use axum::routing::{any, get};
use helpers::{logger::Logger, config::Config};
use helpers::logger::LoggerMode;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().map_err(anyhow::Error::msg)?;
    let logger = Logger::new(config.log_file, LoggerMode::Info);

    let mut router: Router = Router::new().route("/", get(routes_handlers::root()));


    Ok(())
}


