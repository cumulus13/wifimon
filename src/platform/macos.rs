/// macOS Wi-Fi backend.
/// macOS Wi-Fi backend — `airport` CLI. Supports wildcard pattern matching.
///
/// Uses the private `airport` CLI tool (bundled with macOS).
/// No PowerShell, WMI, or COM is used.
use crate::error::WifimonError;
use crate::network::{AccessPoint, Security};
use anyhow::Result;
use chrono::Utc;
use tokio::process::Command;
use tracing::debug;

const AIRPORT: &str =
    "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";

pub async fn detect_wireless_interfaces() -> Result<Vec<String>> {
    let out = Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
        .await
        .map_err(|e| WifimonError::Platform(format!("networksetup failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut ifaces = Vec::new();
    let mut is_wifi = false;

    for line in stdout.lines() {
        let t = line.trim();
        if t.starts_with("Hardware Port:") {
            let lower = t.to_lowercase();
            is_wifi = lower.contains("wi-fi") || lower.contains("airport");
        }
        if is_wifi {
            if let Some(dev) = t.strip_prefix("Device:") {
                ifaces.push(dev.trim().to_string());
                is_wifi = false; // reset until next "Hardware Port:"
            }
        }
    }

    if ifaces.is_empty() {
        ifaces.push("en0".to_string());
    }
    debug!(count = ifaces.len(), "Detected macOS Wi-Fi interfaces");
    Ok(ifaces)
}

/// Scan all interfaces whose name matches `pattern` (wildcard supported).
pub async fn scan(pattern: &str) -> Result<Vec<AccessPoint>> {
    let all = detect_wireless_interfaces().await?;

    let matched: Vec<String> = all
        .into_iter()
        .filter(|name| super::iface_matches(name, pattern))
        .collect();

    if matched.is_empty() {
        return Err(WifimonError::InterfaceNotFound(pattern.to_string()).into());
    }

    let mut aps = Vec::new();
    for iface in &matched {
        match scan_one(iface).await {
            Ok(mut a) => aps.append(&mut a),
            Err(e) => tracing::warn!(iface, error = %e, "Scan failed"),
        }
    }
    Ok(aps)
}

async fn scan_one(iface: &str) -> Result<Vec<AccessPoint>> {
    let out = Command::new(AIRPORT)
        .args(["-s"]) // -s = scan
        .output()
        .await
        .map_err(|e| WifimonError::Platform(format!("airport failed: {e}")))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(WifimonError::ScanFailed {
            iface: iface.to_string(),
            reason: err.trim().to_string(),
        }
        .into());
    }

    parse_airport_text(&String::from_utf8_lossy(&out.stdout), iface)
}

/// Parse `airport -s` text table.
///
/// Column layout (space-separated, SSID may contain spaces):
///   SSID  BSSID  RSSI  CHANNEL  HT  CC  SECURITY(...)
///
/// We locate BSSID by its xx:xx:xx:xx:xx:xx pattern.
fn parse_airport_text(text: &str, iface: &str) -> Result<Vec<AccessPoint>> {
    let mut aps = Vec::new();

    for line in text.lines().skip(1) {
        // skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        // Find BSSID column (exactly 17 chars, 5 colons)
        let Some(bi) = parts
            .iter()
            .position(|p| p.len() == 17 && p.bytes().filter(|&b| b == b':').count() == 5)
        else {
            continue;
        };

        let ssid = parts[..bi].join(" ");
        let bssid = parts[bi].to_uppercase();
        let signal = parts
            .get(bi + 1)
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(-100);
        // channel may be "6" or "6,+1" for HT40
        let channel = parts
            .get(bi + 2)
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.parse::<u32>().ok());
        // security starts after HT and CC columns (bi+4 or later)
        let sec_str = parts.get(bi + 4).copied().unwrap_or("OPEN");
        let security = parse_security(sec_str);

        aps.push(AccessPoint {
            ssid: Some(ssid),
            bssid,
            signal,
            channel,
            frequency: channel.map(channel_to_freq),
            security,
            interface: iface.to_string(),
            last_seen: Utc::now(),
            connected: false,
        });
    }

    Ok(aps)
}

fn parse_security(s: &str) -> Vec<Security> {
    let u = s.to_uppercase();
    if u.contains("WPA3") {
        vec![Security::Wpa3]
    } else if u.contains("WPA2") {
        vec![Security::Wpa2]
    } else if u.contains("WPA") {
        vec![Security::Wpa]
    } else if u.contains("WEP") {
        vec![Security::Wep]
    } else {
        vec![Security::Open]
    }
}

fn channel_to_freq(ch: u32) -> u32 {
    if ch <= 14 {
        2407 + ch * 5
    } else {
        5000 + ch * 5
    }
}
