use axum::extract::State;
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};
use sysinfo::{ProcessesToUpdate, System};
use helpers::logger::{Logger, LoggerMode};
use crate::app_state::AppState;

pub async fn root(State(state): State<AppState>) -> Json<Value> {
    let _ = state.logger.log("root was called");
    Json(json!({ "message": "OK" }))
}

#[derive(Serialize)]
pub struct ProcessInfo {
    pid: u32,
    name: String
}

pub async fn status(State(state): State<AppState>) -> Json<Value> {
    let uptime_sec = state.started_at.elapsed().as_secs();

    let mut sys = state.system.lock().unwrap();
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let process = match sysinfo::get_current_pid() {
        Ok(pid) => {
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            sys.process(pid).map(|p| json!({
                "pid": pid.as_u32(),
                "name": p.name().to_string_lossy(),
                "memory_mb": to_mb(p.memory()),
                "cpu_usage_percent": p.cpu_usage() / sys.cpus().len().max(1) as f32,
            }))
        }
        Err(_) => None,
    };

    Json(json!({
        "uptime_sec": uptime_sec,
        "process": process,
    }))
}
pub fn to_mb(bytes: u64) -> f64 {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    (mb * 10.0).round() / 10.0
}
