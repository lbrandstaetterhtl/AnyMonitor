use std::arch::x86_64::_mm_and_pd;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use axum::extract::{ConnectInfo, Path, State};
use axum::{Json, Router};
use axum::http::HeaderMap;
use axum::routing::{delete, get, head, post};
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

async fn root(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Json<Value> {
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/ was called from {}", addr.ip()).as_str());
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
    let monitor_routes = Router::new()
        .route("/start", post(monitor))
        .route("/stop/{id}", delete(stop_monitor))
        .route("/history/{id}", get(get_entry_history));

    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
        .nest("/monitor", monitor_routes)
        .route("/ui", get(ui))
}

async fn status(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");

    require_key(key, State(state.clone()), addr, "/status")?;

    let uptime_sec = state.started_at.elapsed().as_secs();
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/status was called from {}", addr.ip()).as_str());

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
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, "/monitor")?;

    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/monitor was called from {}", addr.ip()).as_str());


    let strat = req.strategy;
    let target = req.target;
    let interval = req.interval_sec;
    let requester_ipv4 = match addr.ip() {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR)
    };

    match strat {
        MonitorStrat::Icmp => {
            let id = state.start_icmp_monitor(target, requester_ipv4, interval);

            state.logger.set_mode(LoggerMode::Info);
            let _ = state.logger.log(format!("monitor with id '{}' started successfully", id).as_str());

            Ok(Json(json!({"message": "icmp monitor started successfully", "id": id})))
        },
        MonitorStrat::Http {path} => {panic!()},
        MonitorStrat::Tcp {port} => {panic!()},
        MonitorStrat::Heartbeat => {panic!()}
    }
}

async fn get_entry_history(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/history/{}", id).as_str())?;

    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/hsitory/{} was called from {}", id, addr.ip()).as_str());

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

fn require_key(key: &str, State(state): State<AppState>, addr: SocketAddr, path: &str) -> Result<(), StatusCode>{
    if !key.eq(state.config.api_key.as_str()) {
        state.logger.set_mode(LoggerMode::Warn);
        let _ = state.logger.log(format!("Blocked unauthorized request to {} from {}", path, addr.ip()).as_str());
        Err(StatusCode::UNAUTHORIZED)
    }
    else {
        Ok(())
    }
}

use axum::response::Html;

async fn ui(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Html<&'static str> {
    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/ui was called from {}", addr.ip()).as_str());
    Html(include_str!("static/index.html"))
}

async fn stop_monitor(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/monitor/{}", id).as_str())?;

    state.logger.set_mode(LoggerMode::Info);
    let _ = state.logger.log(format!("/monitor/stop/{} was called from {}", id, addr.ip()).as_str());

    match state.stop_icmp_monitor(id) {
        Ok(_) => {
            state.logger.set_mode(LoggerMode::Info);
            let _ = state.logger.log(format!("monitor with id {} successfully stopped", id).as_str());
            Ok(Json(json!({"message": format!("monitor with id {} successfully stopped", id).as_str()})))
        },
        Err(_) => Err(StatusCode::EXPECTATION_FAILED)
    }
}
