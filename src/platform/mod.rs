/// Platform abstraction layer.
///
/// Every backend implements two async functions:
///   detect_wireless_interfaces() -> Result<Vec<String>>
///   scan(iface: &str)            -> Result<Vec<AccessPoint>>
///
/// Interface pattern matching
/// ──────────────────────────
/// The -i argument is matched case-insensitively against interface names.
/// Wildcards * (any sequence) and ? (any single char) are supported via
/// the `wildmatch` crate.  Plain strings also match as substrings.
/// No PowerShell, no WMI/WMIC, no COM is used on any platform.
///   Linux   → `iw` subprocess  (nl80211 via kernel)
///   macOS   → `airport` subprocess (CoreWLAN private framework CLI)
///   Windows → Win32 WLAN API directly via the `windows` crate
///             (WlanOpenHandle / WlanEnumInterfaces / WlanScan /
///              WlanGetNetworkBssList / WlanFreeMemory / WlanCloseHandle)

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

use crate::network::AccessPoint;
use anyhow::Result;
use wildmatch::WildMatch;

// ── Shared wildcard matcher ───────────────────────────────────────────────────

/// Returns true if `name` matches `pattern` (case-insensitive).
/// Supports * and ? wildcards; falls back to substring if no wildcards present.
pub fn iface_matches(name: &str, pattern: &str) -> bool {
    let name_l = name.to_lowercase();
    let pat_l = pattern.to_lowercase();

    // Exact match
    if name_l == pat_l {
        return true;
    }

    // Glob/wildcard match
    if WildMatch::new(&pat_l).matches(&name_l) {
        return true;
    }

    // Substring fallback (only when pattern has no wildcard chars)
    if !pattern.contains('*') && !pattern.contains('?') {
        return name_l.contains(&pat_l);
    }

    false
}

// ── Public surface ────────────────────────────────────────────────────────────

/// Returns the list of wireless interface names for this platform.
/// On Windows these are the friendly names ("Wi-Fi", "Wi-Fi 3").
pub async fn detect_wireless_interfaces() -> Result<Vec<String>> {
    #[cfg(target_os = "linux")]
    {
        return linux::detect_wireless_interfaces().await;
    }
    #[cfg(target_os = "macos")]
    {
        return macos::detect_wireless_interfaces().await;
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_wireless_interfaces().await;
    }
    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("Unsupported platform"))
}

/// Scan the interface(s) matching `iface_pattern`.
/// Pattern may contain * and ? wildcards, or be a plain substring.
pub async fn scan(iface_pattern: &str) -> Result<Vec<AccessPoint>> {
    #[cfg(target_os = "linux")]
    {
        return linux::scan(iface_pattern).await;
    }
    #[cfg(target_os = "macos")]
    {
        return macos::scan(iface_pattern).await;
    }
    #[cfg(target_os = "windows")]
    {
        return windows::scan(iface_pattern).await;
    }
    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("Unsupported platform"))
}

/// Print all wireless interfaces and exit.
pub async fn list_interfaces() -> Result<()> {
    let ifaces = detect_wireless_interfaces().await?;
    if ifaces.is_empty() {
        println!("No wireless interfaces found.");
    } else {
        println!("Wireless interfaces:");
        for i in &ifaces {
            println!("  {i}");
        }
        println!();
        println!("Tip: use any of the above names with -i, or use wildcards:");
        println!("  -i \"Wi-Fi 3\"    (exact / substring)");
        println!("  -i \"*wi*3*\"     (glob pattern)");
        println!("  -i \"*tenda*\"    (glob pattern)");
    }
    Ok(())
}

/// One-shot scan: print table and exit.
pub async fn scan_once(iface: Option<&str>) -> Result<()> {
    let ifaces = match iface {
        Some(i) => {
            // Expand pattern against detected interfaces
            let all = detect_wireless_interfaces().await?;
            let matched: Vec<String> = all
                .into_iter()
                .filter(|name| iface_matches(name, i))
                .collect();
            if matched.is_empty() {
                vec![i.to_string()]
            } else {
                matched
            }
        }
        None => detect_wireless_interfaces().await?,
    };

    for iface in &ifaces {
        let aps = scan(iface).await?;
        crate::ui::print_scan_table(&aps, iface);
    }
    Ok(())
}
