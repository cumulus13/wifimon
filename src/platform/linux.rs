/// Linux Wi-Fi backend.
/// Linux Wi-Fi backend — `iw` subprocess + /proc/net/wireless.
/// Supports wildcard pattern matching on interface names.
///
/// Uses `iw dev <iface> scan` to trigger a fresh scan and parses its
/// text output.  Falls back to `iw dev <iface> scan dump` (cached
/// results) when the fresh scan fails (e.g. because the caller is
/// unprivileged).
///
/// Interface detection reads /proc/net/wireless first, then falls back
/// to `iw dev`.
///
/// No PowerShell, WMI, or COM is used.
use crate::error::WifimonError;
use crate::network::{AccessPoint, Security};
use anyhow::Result;
use chrono::Utc;
use tokio::process::Command;
use tracing::{debug, warn};

// ── Interface detection ───────────────────────────────────────────────────────

pub async fn detect_wireless_interfaces() -> Result<Vec<String>> {
    // Primary: /proc/net/wireless (available on any kernel with cfg80211)
    if let Ok(content) = tokio::fs::read_to_string("/proc/net/wireless").await {
        let ifaces: Vec<String> = content
            .lines()
            .skip(2)
            .filter_map(|l| l.trim().split(':').next().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        if !ifaces.is_empty() {
            debug!(count = ifaces.len(), "Interfaces via /proc/net/wireless");
            return Ok(ifaces);
        }
    }

    // Fallback: `iw dev`
    let out = Command::new("iw")
        .args(["dev"])
        .output()
        .await
        .map_err(|e| WifimonError::Platform(format!("`iw dev` failed: {e}")))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let ifaces: Vec<String> = stdout
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("Interface ")
                .map(|s| s.trim().to_string())
        })
        .collect();
    debug!(count = ifaces.len(), "Interfaces via `iw dev`");
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
            Err(e) => warn!(iface, error = %e, "Scan failed"),
        }
    }
    Ok(aps)
}

async fn scan_one(iface: &str) -> Result<Vec<AccessPoint>> {
    match run_iw_scan(iface).await {
        Ok(aps) => Ok(aps),
        Err(e) => {
            warn!(iface, error = %e, "Fresh scan failed, trying dump");
            run_iw_dump(iface).await
        }
    }
}

async fn run_iw_scan(iface: &str) -> Result<Vec<AccessPoint>> {
    let out = Command::new("iw")
        .args(["dev", iface, "scan"])
        .output()
        .await
        .map_err(|e| WifimonError::Platform(format!("`iw scan` unavailable: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(WifimonError::ScanFailed {
            iface: iface.to_string(),
            reason: err.trim().to_string(),
        }
        .into());
    }
    parse_iw_output(&String::from_utf8_lossy(&out.stdout), iface)
}

async fn run_iw_dump(iface: &str) -> Result<Vec<AccessPoint>> {
    let out = Command::new("iw")
        .args(["dev", iface, "scan", "dump"])
        .output()
        .await
        .map_err(|e| WifimonError::Platform(format!("`iw scan dump` unavailable: {e}")))?;
    parse_iw_output(&String::from_utf8_lossy(&out.stdout), iface)
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_iw_output(text: &str, iface: &str) -> Result<Vec<AccessPoint>> {
    let mut aps: Vec<AccessPoint> = Vec::new();
    let mut current: Option<AccessPoint> = None;

    for line in text.lines() {
        let t = line.trim();

        // New BSS block
        if t.starts_with("BSS ") {
            if let Some(ap) = current.take() {
                aps.push(ap);
            }

            let bssid = t
                .strip_prefix("BSS ")
                .and_then(|s| s.split(|c| c == '(' || c == ' ').next())
                .unwrap_or("")
                .trim()
                .to_uppercase();

            current = Some(AccessPoint {
                ssid: None,
                bssid,
                signal: -100,
                channel: None,
                frequency: None,
                security: vec![Security::Open],
                interface: iface.to_string(),
                last_seen: Utc::now(),
                connected: t.contains("(on "),
            });
            continue;
        }

        let Some(ref mut ap) = current else { continue };

        if let Some(v) = t.strip_prefix("SSID:") {
            ap.ssid = Some(v.trim().to_string());
        } else if let Some(v) = t.strip_prefix("signal:") {
            // e.g. "signal: -67.00 dBm"
            if let Some(s) = v.trim().split_whitespace().next() {
                if let Ok(f) = s.parse::<f32>() {
                    ap.signal = f as i32;
                }
            }
        } else if let Some(v) = t.strip_prefix("freq:") {
            if let Ok(mhz) = v.trim().parse::<u32>() {
                ap.frequency = Some(mhz);
                ap.channel = Some(freq_to_channel(mhz));
            }
        } else if t.contains("RSN:") || t.contains("WPA2") {
            ap.security = vec![Security::Wpa2];
        } else if t.contains("WPA3") {
            ap.security = vec![Security::Wpa3];
        } else if t.contains("WPA:") && ap.security == vec![Security::Open] {
            ap.security = vec![Security::Wpa];
        } else if t.contains("Privacy") && ap.security == vec![Security::Open] {
            ap.security = vec![Security::Wep];
        }
    }
    if let Some(ap) = current {
        aps.push(ap);
    }
    Ok(aps)
}

fn freq_to_channel(mhz: u32) -> u32 {
    match mhz {
        2412..=2472 => (mhz - 2407) / 5,
        2484 => 14,
        5180..=5885 => (mhz - 5000) / 5,
        _ => 0,
    }
}
