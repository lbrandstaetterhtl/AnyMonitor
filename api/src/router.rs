use crate::app_state::AppState;
use crate::monitor::{MonitorInfo, MonitorStrat};
use crate::requests::MonitorReq;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use helpers::logger::LoggerMode;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use std::net::{IpAddr, SocketAddr};
use sysinfo::ProcessesToUpdate;
use uuid::Uuid;

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
        .route("/history/{id}", get(get_entry_history))
        .route("/info/{id}", get(get_entry_info))
        .route("/info/all", get(get_all_entries))
        .route("/pause/{id}", post(pause_loop))
        .route("/resume/{id}", post(resume_loop));

    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
        .nest("/monitor", monitor_routes)
        .route("/ui", get(ui))
}

async fn status(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");

    require_key(key, State(state.clone()), addr, "/status".to_string()).log_and_return(&state.logger)?;

    let uptime_sec = state.started_at.elapsed().as_secs();
    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/status was called from {}", addr.ip()).as_str());

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

    let loop_count = state.running_loops.lock().unwrap().len();
    Ok(Json(json!({
        "uptime_sec": uptime_sec,
        "process": process,
        "running_loops": loop_count
    })))
}

async fn monitor(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(req): Json<MonitorReq>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, "/monitor/start".to_string()).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/start was called from {}", addr.ip()).as_str());


    let strat = req.strategy;
    let target = req.target;
    let interval = req.interval_sec;
    let requester_ipv4 = match addr.ip() {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR)
    };

    if interval < 5 {
        return Err(StatusCode::BAD_REQUEST)
    }

    match strat {
        MonitorStrat::Icmp => {
            let id = state.start_icmp_monitor(target, requester_ipv4, interval)?;

            state.logger.set_mode(LoggerMode::Info);
            state.logger.log(format!("monitor with id '{}' started successfully", id).as_str());

            Ok(Json(json!({"message": "icmp monitor started successfully", "id": id})))
        },
        MonitorStrat::Http {path} => {Err(StatusCode::NOT_IMPLEMENTED)},
        MonitorStrat::Tcp {port} => {Err(StatusCode::NOT_IMPLEMENTED)},
        MonitorStrat::Heartbeat => {Err(StatusCode::NOT_IMPLEMENTED)}
    }
}

async fn get_entry_history(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("monitor//history/{}", id)).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/hsitory/{} was called from {}", id, addr.ip()).as_str());

    let entry = state.get_entry(id).log_and_return(&state.logger)?;

    Ok(Json(json!({"history": entry.history})))
}

async fn get_entry_info(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode>{
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/monitor/info/{}", id)).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/info/{} was called from {}", id, addr.ip()).as_str());

    let entry = state.get_entry(id).log_and_return(&state.logger)?;

    let info = MonitorInfo::from_entry(id, &entry);

    Ok(Json(json!({"info": info})))
}

fn to_mb(bytes: u64) -> f64 {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    (mb * 10.0).round() / 10.0
}

fn require_key(key: &str, State(state): State<AppState>, addr: SocketAddr, path: String) -> Result<(), Errors>{
    if key.is_empty() {
        Err(Errors::NoApiKey {ip: addr.ip(), path})
    }
    else if !key.eq(state.config.api_key.as_str()) {
        Err(Errors::InvalidApiKey {ip: addr.ip(), path})
    }
    else {
        Ok(())
    }
}

use crate::errors::{Errors, LogAndReturn};
use axum::response::Html;

async fn ui(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Result<Html<&'static str>, StatusCode> {
    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/ui was called from {}", addr.ip()).as_str());
    Ok(Html(include_str!("static/index.html")))
}

async fn stop_monitor(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/monitor/{}", id)).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/stop/{} was called from {}", id, addr.ip()).as_str());

    match state.stop_monitor(id) {
        Ok(_) => {
            state.logger.set_mode(LoggerMode::Info);
            state.logger.log(format!("monitor with id {} successfully stopped", id).as_str());
            Ok(Json(json!({"message": format!("monitor with id {} successfully stopped", id).as_str()})))
        },
        Err(e) => Err(e.to_status_code())
    }
}

async fn get_all_entries(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, "/monitor/info/all".to_string()).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/info/all was called from {}", addr.ip()).as_str());

    let mut infos: Vec<MonitorInfo> = Vec::new();

    let map = state.monitor_entries.lock().unwrap().clone();

    for (id, entry) in map {
        let info = MonitorInfo::from_entry(id, &entry);
        infos.push(info);
    }

    Ok(Json(json!({"infos": infos})))
}

async fn pause_loop(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/monitor/pause/{}", id)).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/pause/{} was called from {}", id, addr.ip()).as_str());

    state.pause_monitor(id).log_and_return(&state.logger)?;

    Ok(Json(json!({"message": "loop paused successfully"})))
}

async fn resume_loop(State(state): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>, StatusCode> {
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_else(|| "");
    require_key(key, State(state.clone()), addr, format!("/monitor/resume/{}", id)).log_and_return(&state.logger)?;

    state.logger.set_mode(LoggerMode::Info);
    state.logger.log(format!("/monitor/resume/{} was called from {}", id, addr.ip()).as_str());

    state.resume_monitor(id).log_and_return(&state.logger)?;

    Ok(Json(json!({"message": "resume successfully"})))
}
