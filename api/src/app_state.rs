use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use sysinfo::System;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use axum::http::StatusCode;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use uuid::Uuid;
use helpers::config::Config;
use helpers::logger::{Logger};
use helpers::monitor::{MonitorEntry, PingResult};
use tokio_util::sync::CancellationToken;
use helpers::monitor;
use helpers::monitor::MonitorStrat;
use crate::errors::{LogAndReturn, Errors};


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
    pub fn start_icmp_monitor(&self, target: Ipv4Addr, requester: Ipv4Addr, interval_sec: u64) -> Result<Uuid, StatusCode> {
        let id = Uuid::new_v4();

        let (events, _) = broadcast::channel(16);

        let mut map = self.monitor_entries.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
        
        self.running_loops.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.push(icmp_loop);
        
        Ok(id)
    }

    pub fn get_entry(&self, id: Uuid) -> Result<MonitorEntry, Errors> {
        let map = self.monitor_entries.lock().unwrap();

        let Some(entry) = map.get(&id) else {
            return Err(Errors::EntryNotFound {id});
        };

        Ok(entry.clone())
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

    pub fn get_loop_ct(&self, id: Uuid) -> Result<CancellationToken, Errors> {
        let loops = self.running_loops.lock().unwrap();

        let Some(running) = loops.iter().find(|l| l.id == id) else {
            return Err(Errors::LoopNotFound {id});
        };

        Ok(running.ct.clone())
    }

    pub fn stop_monitor(&self, id: Uuid) -> Result<(), Errors> {
        let mut map = self.monitor_entries.lock().unwrap();
        let mut loops = self.running_loops.lock().unwrap();

        if map.remove(&id).is_none() {
            return Err(Errors::EntryNotFound { id });
        }

        let Some(index) = loops.iter().position(|l| l.id == id) else {
            return Err(Errors::LoopNotFound { id });
        };

        loops.remove(index).ct.cancel();
        Ok(())
    }
    
}

pub struct Loop {
    pub id: Uuid,
    pub ct: CancellationToken,
    pub handle: JoinHandle<Result<(), StatusCode>>
}
impl Loop {
    pub async fn icmp_loop(id: Uuid, state: AppState) -> Result<(), StatusCode>{
        let (ip, interval, cancel) = {

            let entry = state.get_entry(id).log_and_return(&state.logger)?;

            let ct = state.get_loop_ct(id).log_and_return(&state.logger)?;

            (entry.ip, entry.interval_sec, ct.clone())
        };

        let mut ticker = tokio::time::interval(Duration::from_secs(interval));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = ticker.tick() => {
                let result = monitor::full_icmp_check(ip).await;
                state.record_result(id, result);
            }
        }
        }
    }
}
