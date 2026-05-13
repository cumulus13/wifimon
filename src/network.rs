use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Security {
    Open,
    Wep,
    Wpa,
    Wpa2,
    Wpa3,
    Unknown(String),
}

impl fmt::Display for Security {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Security::Open => write!(f, "Open"),
            Security::Wep => write!(f, "WEP"),
            Security::Wpa => write!(f, "WPA"),
            Security::Wpa2 => write!(f, "WPA2"),
            Security::Wpa3 => write!(f, "WPA3"),
            Security::Unknown(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPoint {
    pub ssid: Option<String>,
    pub bssid: String,
    pub signal: i32, // dBm
    pub channel: Option<u32>,
    pub frequency: Option<u32>, // MHz
    pub security: Vec<Security>,
    pub interface: String,
    pub last_seen: DateTime<Utc>,
    pub connected: bool,
}

impl AccessPoint {
    pub fn display_name(&self) -> String {
        match &self.ssid {
            Some(s) if !s.is_empty() => s.clone(),
            _ => format!("<hidden {}>", self.bssid),
        }
    }

    pub fn signal_quality(&self) -> u8 {
        let clamped = self.signal.clamp(-90, -30);
        ((clamped + 90) as f32 / 60.0 * 100.0) as u8
    }

    pub fn signal_label(&self) -> &'static str {
        match self.signal {
            s if s >= -50 => "Excellent",
            s if s >= -60 => "Good",
            s if s >= -70 => "Fair",
            s if s >= -80 => "Poor",
            _ => "Very Poor",
        }
    }
}

impl PartialEq for AccessPoint {
    fn eq(&self, other: &Self) -> bool {
        self.bssid == other.bssid && self.interface == other.interface
    }
}
impl Eq for AccessPoint {}

impl std::hash::Hash for AccessPoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bssid.hash(state);
        self.interface.hash(state);
    }
}

#[derive(Debug, Default)]
pub struct ScanDiff {
    pub new_aps: Vec<AccessPoint>,
    pub lost_aps: Vec<AccessPoint>,
    pub signal_changes: Vec<SignalChange>,
    pub any_change: bool,
}

#[derive(Debug, Clone)]
pub struct SignalChange {
    pub ap: AccessPoint,
    pub old_signal: i32,
    pub delta: i32,
}

impl SignalChange {
    pub fn direction(&self) -> &'static str {
        if self.delta > 0 {
            "▲"
        } else {
            "▼"
        }
    }
}
