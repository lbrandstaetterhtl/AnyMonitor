use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use sysinfo::System;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use uuid::Uuid;
use helpers::config::Config;
use helpers::logger::Logger;
use helpers::monitor::{MonitorEntry, PingResult};
use tokio_util::sync::CancellationToken;
use helpers::monitor;
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
    pub shutdown: CancellationToken,
    pub running_loops: Arc<Mutex<Vec<Loop>>>
}

impl AppState {
    pub fn start_icmp_monitor(&self, target: Ipv4Addr, requester: Ipv4Addr, interval_sec: u64) -> Uuid {
        let id = Uuid::new_v4();

        let (events, _) = broadcast::channel(16);

        let mut map = self.monitor_entries.lock().unwrap();
        let entry = MonitorEntry {
            ip: target,
            ip_requester: requester,
            interval_sec,
            history: VecDeque::new(),
            events,
            strategy: MonitorStrat::Icmp
        };
        map.insert(id, entry);

        let handle = tokio::spawn(Loop::icmp_loop(id, self.clone()));
        let ct = self.shutdown.child_token();
        let icmp_loop = Loop {id, handle, ct};
        
        self.running_loops.lock().unwrap().push(icmp_loop);
        
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
    
    pub fn stop_icmp_monitor(&self, id: Uuid) -> Result<(), String> {
        let mut map = self.monitor_entries.lock().unwrap();
        map.remove(&id);
        
        let mut running_loops = self.running_loops.lock().unwrap();
        let icmp_loop = running_loops.iter().find(|l| l.id == id).ok_or(format!("Loop with id {} not found", id))?;
        let index = running_loops.iter().position(|l| l.id == id).unwrap();
        
        icmp_loop.ct.cancel();
        running_loops.remove(index);
        
        Ok(())
    }
    
}

pub struct Loop {
    pub id: Uuid,
    pub ct: CancellationToken,
    pub handle: JoinHandle<()>,
}
impl Loop {
    pub async fn icmp_loop(id: Uuid, state: AppState) {
        let (ip, interval, cancel) = {
            let entries = state.monitor_entries.lock().unwrap();
            let Some(entry) = entries.get(&id) else {
                let _ = state.logger.log(&format!("monitor {id} not found, loop not started"));
                return;
            };

            let loops = state.running_loops.lock().unwrap();
            let Some(running) = loops.iter().find(|l| l.id == id) else {
                let _ = state.logger.log(&format!("loop {id} not found, loop not started"));
                return;
            };

            (entry.ip, entry.interval_sec, running.ct.clone())
        };

        let mut ticker = tokio::time::interval(Duration::from_secs(interval));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
            _ = cancel.cancelled() => break,
            _ = ticker.tick() => {
                let result = monitor::full_icmp_check(ip).await;
                state.record_result(id, result);
            }
        }
        }
    }
}
