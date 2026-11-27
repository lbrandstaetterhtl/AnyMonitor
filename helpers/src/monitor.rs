use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MonitorStrat {
    Icmp,
    Http { path: String },
    Tcp { port: u16},
    Heartbeat
}

#[derive(Clone)]
pub struct MonitorEntry {
    pub ip: Ipv4Addr,
    pub ip_requester: Ipv4Addr,
    pub interval_sec: u64,
    pub history: VecDeque<PingResult>,
    pub events: broadcast::Sender<PingResult>,
    pub strategy: MonitorStrat
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub at: DateTime<Utc>,
    pub sent: u32,
    pub received: u32,
    pub loss_percent: f32,
    pub avg_rtt_ms: Option<f64>,
    pub errors: Vec<Option<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: Uuid,
    pub ip: Ipv4Addr,
    pub ip_requester: Ipv4Addr,
    pub interval_sec: u64,
    pub strategy: MonitorStrat,
    pub results: usize,
    pub latest: Option<PingResult>,
}

impl MonitorInfo {
    pub fn from_entry(id: Uuid, entry: &MonitorEntry) -> Self {
        Self {
            id,
            ip: entry.ip,
            ip_requester: entry.ip_requester,
            interval_sec: entry.interval_sec,
            strategy: entry.strategy.clone(),
            results: entry.history.len(),
            latest: entry.history.back().cloned(),
        }
    }
}

const PING_PER_CHECK: u32 = 3;
const PING_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn ping(ip: Ipv4Addr) -> Result<Duration, String> {
    let payload = [0u8, 32];

    match tokio::time::timeout(PING_TIMEOUT, surge_ping::ping(IpAddr::V4(ip), &payload)).await {
        Ok(Ok((_, rtt))) => Ok(rtt),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("timeout".to_string())
    }
}

pub async fn full_icmp_check(ip: Ipv4Addr) -> PingResult {
    let mut rtts: Vec<Duration> = Vec::new();
    let mut errors: Vec<Option<String>> = Vec::new();

    for _ in 0..PING_PER_CHECK {
        match ping(ip).await {
            Ok(rtt) => {
                rtts.push(rtt);
                errors.push(None);
            },
            Err(e) => errors.push(Some(e))
        }
    }

    let received = rtts.len() as u32;
    let avg_rtt_ms = if received == 0 {
        None
    }
    else {
        let total: Duration = rtts.iter().sum();
        Some(total.as_secs_f64() * 1000.0 / received as f64)
    };

    PingResult {
        at: Utc::now(),
        sent: PING_PER_CHECK,
        received,
        loss_percent: (PING_PER_CHECK - received) as f32 / PING_PER_CHECK as f32 * 100.0,
        avg_rtt_ms,
        errors
    }
}