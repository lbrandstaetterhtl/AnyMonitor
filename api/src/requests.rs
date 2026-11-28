use crate::monitor::MonitorStrat;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub struct MonitorReq {
    pub target: Ipv4Addr,
    pub strategy: MonitorStrat,
    pub interval_sec: u64,
    pub timeout_ms: u64,
}