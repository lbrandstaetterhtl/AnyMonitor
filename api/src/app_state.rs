use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use sysinfo::System;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;
use helpers::config::Config;
use helpers::logger::Logger;
use helpers::monitor::{MonitorEntry, PingResult};
use tokio_util::sync::CancellationToken;
use crate::router;
use helpers::monitor::MonitorStrat;


const HISTORY_LIMIT: usize = 100;
#[derive(Clone)]
pub struct AppState {
    pub logger: Arc<Logger>,
    pub config: Arc<Config>,
    pub started_at: Instant,
    pub system: Arc<Mutex<System>>,
    pub monitor_entries: Arc<Mutex<HashMap<Uuid, MonitorEntry>>>,
    pub shutdown: CancellationToken
}

impl AppState {
    pub fn start_icmp_monitor(&self, target: Ipv4Addr, requester: Ipv4Addr, interval_sec: u64) -> Uuid {
        let id = Uuid::new_v4();

        let (events, _) = broadcast::channel(16);

        let mut map = self.monitor_entries.lock().unwrap();
        let handle = tokio::spawn(router::icmp_loop(id, target, interval_sec, self.clone()));
        map.insert(id, MonitorEntry {
            ip: target,
            ip_requester: requester,
            interval_sec,
            history: VecDeque::new(),
            handle,
            events,
            strategy: MonitorStrat::Icmp
        });
        id
    }

    pub fn record_result(&self, id: Uuid, result: PingResult) {
        let mut map = self.monitor_entries.lock().unwrap();
        if let Some(entry) = map.get_mut(&id) {
            if entry.history.len() >= HISTORY_LIMIT {
                entry.history.pop_front();
            }
            
            entry.history.push_back(result);
        }
    }
}