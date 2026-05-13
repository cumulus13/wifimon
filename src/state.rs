use crate::error::WifimonError;
use crate::network::AccessPoint;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAp {
    pub ssid: Option<String>,
    pub bssid: String,
    pub last_signal: i32,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub times_seen: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistentState {
    pub version: u32,
    pub known: HashMap<String, KnownAp>,
    pub last_scan: Option<DateTime<Utc>>,
}

impl PersistentState {
    const VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            known: HashMap::new(),
            last_scan: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            debug!(path = %path.display(), "State file not found, starting fresh");
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(path).map_err(WifimonError::StateIo)?;
        let state: Self = serde_json::from_str(&raw).map_err(WifimonError::StateJson)?;
        if state.version != Self::VERSION {
            warn!(
                file_version = state.version,
                "State version mismatch — resetting"
            );
            return Ok(Self::new());
        }
        debug!(known = state.known.len(), "Loaded persistent state");
        Ok(state)
    }

    /// Atomic write: tmp file then rename
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(WifimonError::StateIo)?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(self).map_err(WifimonError::StateJson)?;
        std::fs::write(&tmp, json).map_err(WifimonError::StateIo)?;
        std::fs::rename(&tmp, path).map_err(WifimonError::StateIo)?;
        debug!(path = %path.display(), "Saved persistent state");
        Ok(())
    }

    pub fn update_ap(&mut self, ap: &AccessPoint) -> bool {
        let now = Utc::now();
        let is_new = !self.known.contains_key(&ap.bssid);
        let entry = self
            .known
            .entry(ap.bssid.clone())
            .or_insert_with(|| KnownAp {
                ssid: ap.ssid.clone(),
                bssid: ap.bssid.clone(),
                last_signal: ap.signal,
                first_seen: now,
                last_seen: now,
                times_seen: 0,
            });
        entry.last_signal = ap.signal;
        entry.last_seen = now;
        entry.times_seen += 1;
        if ap.ssid.is_some() {
            entry.ssid = ap.ssid.clone();
        }
        is_new
    }
}

#[derive(Debug, Default)]
pub struct ScanState {
    pub current: HashMap<String, AccessPoint>,
    pub persistent: Option<PersistentState>,
    pub state_path: Option<PathBuf>,
}

impl ScanState {
    pub fn new(state_path: Option<PathBuf>) -> Result<Self> {
        let persistent = match &state_path {
            Some(p) => Some(PersistentState::load(p)?),
            None => None,
        };
        Ok(Self {
            current: HashMap::new(),
            persistent,
            state_path,
        })
    }

    pub fn save_if_needed(&self) -> Result<()> {
        if let (Some(state), Some(path)) = (&self.persistent, &self.state_path) {
            state.save(path)?;
        }
        Ok(())
    }
}
