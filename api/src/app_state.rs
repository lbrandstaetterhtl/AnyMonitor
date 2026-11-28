use crate::errors::{Errors, LogAndReturn};
use crate::monitor;
use crate::monitor::{Loop, MonitorStrat, StratToLoop};
use crate::monitor::{MonitorEntry, PingResult};
use axum::http::StatusCode;
use helpers::config::Config;
use helpers::logger::{Logger, LoggerMode};
use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;


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
    pub fn start_icmp_monitor(&self, target: Ipv4Addr, requester: Ipv4Addr, interval_sec: u64, timeout: Duration) -> Result<Uuid, StatusCode> {
        let id = Uuid::new_v4();

        let (events, _) = broadcast::channel(16);

        let log_file = format!("loop-{}.log", id);
        let mut map = self.monitor_entries.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let entry = MonitorEntry {
            ip: target,
            ip_requester: requester,
            interval_sec,
            history: VecDeque::new(),
            events,
            strategy: MonitorStrat::Icmp,
            paused: false,
            timeout,
            log_file
        };
        map.insert(id, entry);

        drop(map);

        self.spawn_loop(id).log_and_return(&self.logger)?;
        
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
        let entry = self.get_entry(id)?;

        let mut map = self.monitor_entries.lock().unwrap();
        let mut loops = self.running_loops.lock().unwrap();

        self.logger.set_mode(LoggerMode::Info);
        self.logger.monitor_log(format!("monitor with id {} was stopped and removed", id).as_str(), entry.log_file.clone());

        if map.remove(&id).is_none() {
            return Err(Errors::EntryNotFound { id });
        }

        if let Some(index) = loops.iter().position(|l| l.id == id) {
            loops.remove(index).ct.cancel();
        }

        Ok(())
    }

    pub fn pause_monitor(&self, id: Uuid) -> Result<(), Errors> {
        let entry = self.get_entry(id)?;

        let mut map = self.monitor_entries.lock().unwrap();
        let mut loops = self.running_loops.lock().unwrap();

        self.logger.set_mode(LoggerMode::Info);
        self.logger.monitor_log(format!("monitor with id {} was paused", id).as_str(), entry.log_file.clone());

        let Some(entry) = map.get_mut(&id) else {
            return Err(Errors::EntryNotFound { id });
        };

        if entry.paused {
            return Err(Errors::AlreadyPaused { id });
        }
        let Some(index) = loops.iter().position(|l| l.id == id) else {
            return Err(Errors::LoopNotFound { id });
        };

        loops.remove(index).ct.cancel();
        entry.set_paused(true);
        Ok(())
    }

    pub fn resume_monitor(&self, id: Uuid) -> Result<(), Errors> {
        {
            let mut map = self.monitor_entries.lock().unwrap();

            let Some(entry) = map.get_mut(&id) else {
                return Err(Errors::EntryNotFound { id });
            };

            if !entry.paused {
                return Err(Errors::NotPaused { id });
            }

            entry.set_paused(false);

            self.logger.set_mode(LoggerMode::Info);
            self.logger.monitor_log(format!("monitor with id {} was resumed", id).as_str(), entry.log_file.clone());
        }

        self.spawn_loop(id)?;
        Ok(())
    }

    fn spawn_loop(&self, id: Uuid,) -> Result<(), Errors> {
        let strategy = self.get_entry(id)?.strategy;
        let mut loops = self.running_loops.lock().unwrap();
        let ct = self.shutdown.child_token();
        let handle = tokio::spawn(strategy.clone().run_loop(id, self.clone()));
        loops.push(Loop { id, handle, ct});
        Ok(())
    }
}


