use std::net::Ipv4Addr;
use serde::{Deserialize, Serialize};
use helpers::monitor::MonitorStrat;

#[derive(Serialize, Deserialize)]
pub struct MonitorReq {
    pub target: Ipv4Addr,
    pub strategy: MonitorStrat,
    pub interval_sec: u64
}