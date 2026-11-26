use std::arch::x86_64::_mm_and_pd;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use axum::extract::{ConnectInfo, Path, State};
use axum::{Json, Router};
use axum::http::HeaderMap;
use axum::routing::{get, head, post};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use sysinfo::{ProcessesToUpdate, System};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;
use helpers::logger::{Logger, LoggerMode};
use helpers::monitor;
use helpers::monitor::MonitorStrat;
use crate::app_state::AppState;
use crate::requests::{MonitorReq};
use crate::router;

async fn root(State(state): State<AppState>) -> Json<Value> {
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log("root was called");
    Json(json!({ "message": "OK" }))
}

#[derive(Serialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    memory_mb: f64,
    cpu_usage_percent: f32
}

pub fn get_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
        .route("/monitor", post(monitor))
        .route("/history/{id}", get(get_monitor_entry))
        .route("/ui", get(ui))
}

async fn status(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");

    require_key(key, State(state.clone()), addr)?;

    let uptime_sec = state.started_at.elapsed().as_secs();
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log("status was called");

    let mut sys = state.system.lock().unwrap();
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let process = match sysinfo::get_current_pid() {
        Ok(pid) => {
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            sys.process(pid).map(|p| ProcessInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                memory_mb: to_mb(p.memory()),
                cpu_usage_percent: p.cpu_usage() / sys.cpus().len().max(1) as f32,
            })
        }
        Err(_) => None,
    };

    let message = match &process {
        Some(p) => format!(
            "status: uptime={}s memory={}MB cpu={:.1}%",
            uptime_sec, p.memory_mb, p.cpu_usage_percent
        ),
        None => format!("status: uptime={}s, process info unavailable", uptime_sec),
    };
    let _ = state.logger.log(&message);

    Ok(Json(json!({
        "uptime_sec": uptime_sec,
        "process": process,
    })))
}

async fn monitor(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(req): Json<MonitorReq>) -> Result<Json<Value>, StatusCode> {
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log("/monitor was called");
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");

    require_key(key, State(state.clone()), addr)?;

    let strat = req.strategy;
    let target = req.target;
    let interval = req.interval_sec;

    match strat {
        MonitorStrat::Icmp => {
            let id = state.start_icmp_monitor(target, interval);

            state.logger.set_mode(LoggerMode::Info);
            let _ = state.logger.log(format!("monitor with id '{}' started successfully", id).as_str());

            Ok(Json(json!({"message": "icmp monitor started successfully", "id": id})))
        },
        MonitorStrat::Http {path} => {panic!()},
        MonitorStrat::Tcp {port} => {panic!()},
        MonitorStrat::Heartbeat => {panic!()}
    }
}

async fn get_monitor_entry(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");

    require_key(key, State(state.clone()), addr)?;

    let map = state.monitor_entries.lock().unwrap();

    let Some(entry) = map.get(&id) else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(Json(json!({"history": &entry.history})))
}

fn to_mb(bytes: u64) -> f64 {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    (mb * 10.0).round() / 10.0
}

fn require_key(key: &str, State(state): State<AppState>, addr: SocketAddr) -> Result<(), StatusCode>{
    if !key.eq(state.config.api_key.as_str()) {
        state.logger.set_mode(LoggerMode::Warn);
        let _ = state.logger.log(format!("Unauthorized request to /status from {}", addr.ip()).as_str());
        Err(StatusCode::UNAUTHORIZED)
    }
    else {
        Ok(())
    }
}

pub async fn icmp_loop(id: Uuid, ip: Ipv4Addr, interval: u64, state: AppState) {
    let token = state.shutdown.clone();
    let mut ticker = tokio::time::interval(Duration::from_secs(interval));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            _ = ticker.tick() => {
                let result = monitor::full_icmp_check(ip).await;
                state.record_result(id, result);
            }
        }
    }
}

use axum::response::Html;

async fn ui() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}
