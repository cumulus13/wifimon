/// Windows Wi-Fi backend.
///
/// Uses the Win32 WLAN API exclusively (wlanapi.dll).
/// No PowerShell, WMI, WMIC, COM, or any managed runtime.
///
/// Interface names
/// ───────────────
/// WlanEnumInterfaces returns WLAN_INTERFACE_INFO which only carries
/// strInterfaceDescription (hardware description, e.g. "Intel(R) Dual Band
/// Wireless-AC 8265").  The user-visible friendly name ("Wi-Fi", "Wi-Fi 3")
/// is stored in the registry under:
///   HKLM\SYSTEM\CurrentControlSet\Control\Network\
///     {4D36E972-E325-11CE-BFC1-08002BE10318}\<GUID>\Connection\Name
///
/// We read that key for every GUID so that -i matches against BOTH the
/// friendly name AND the hardware description.
///
/// Wildcard / glob matching
/// ────────────────────────
/// The pattern passed via -i is matched case-insensitively with the
/// `wildmatch` crate (* = any sequence, ? = any single char).
/// If the pattern contains no wildcard characters it falls back to a
/// plain case-insensitive substring match for ergonomics.
#[cfg(target_os = "windows")]
use windows::{
    core::GUID,
    Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE},
        NetworkManagement::WiFi::{
            dot11_BSS_type_any, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory,
            WlanGetNetworkBssList, WlanOpenHandle, WlanScan, WLAN_INTERFACE_INFO_LIST,
        },
    },
};

use crate::error::WifimonError;
use crate::network::{AccessPoint, Security};
use anyhow::Result;
use chrono::Utc;
use tracing::debug;
use wildmatch::WildMatch;

// ── Public surface ────────────────────────────────────────────────────────────

pub async fn detect_wireless_interfaces() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(interfaces_sync)
        .await
        .map_err(|e| WifimonError::Platform(e.to_string()))?
}

pub async fn scan(iface_pattern: &str) -> Result<Vec<AccessPoint>> {
    let pattern = iface_pattern.to_string();
    tokio::task::spawn_blocking(move || scan_sync(&pattern))
        .await
        .map_err(|e| WifimonError::Platform(e.to_string()))?
}

// ── Interface record ──────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct IfaceInfo {
    guid: GUID,
    friendly: String,    // "Wi-Fi", "Wi-Fi 3" — from registry
    description: String, // "Intel(R) Dual Band Wireless-AC 8265" — from WLAN API
}

#[cfg(target_os = "windows")]
impl IfaceInfo {
    /// Does this interface match a user-supplied pattern?
    ///
    /// Matching rules (all case-insensitive):
    ///  1. Exact match against friendly name or description
    ///  2. Substring match (no wildcards in pattern)
    ///  3. Glob/wildcard match via wildmatch (* and ?)
    fn matches(&self, pattern: &str) -> bool {
        let pat_lower = pattern.to_lowercase();
        let fri_lower = self.friendly.to_lowercase();
        let desc_lower = self.description.to_lowercase();

        // Exact
        if fri_lower == pat_lower || desc_lower == pat_lower {
            return true;
        }

        // Wildcard glob
        let wm = WildMatch::new(&pat_lower);
        if wm.matches(&fri_lower) || wm.matches(&desc_lower) {
            return true;
        }

        // Plain substring fallback (only when no wildcard chars present)
        if !pattern.contains('*')
            && !pattern.contains('?')
            && (fri_lower.contains(&pat_lower) || desc_lower.contains(&pat_lower))
        {
            return true;
        }

        false
    }
}

// ── RAII session handle ───────────────────────────────────────────────────────
//
// Wraps a WLAN client handle so it is always closed, even on early return.

#[cfg(target_os = "windows")]
struct WlanSession(HANDLE);

#[cfg(target_os = "windows")]
impl WlanSession {
    fn open() -> Result<Self> {
        let mut version = 0u32;
        let mut handle = HANDLE::default();
        let rc = unsafe { WlanOpenHandle(2, None, &mut version, &mut handle) };
        if rc != ERROR_SUCCESS.0 {
            return Err(
                WifimonError::Platform(format!("WlanOpenHandle failed: error {rc}")).into(),
            );
        }
        debug!(negotiated_version = version, "WLAN session opened");
        Ok(Self(handle))
    }
    fn raw(&self) -> HANDLE {
        self.0
    }
}

#[cfg(target_os = "windows")]
impl Drop for WlanSession {
    fn drop(&mut self) {
        unsafe { WlanCloseHandle(self.0, None) };
        debug!("WLAN session closed");
    }
}

// ── RAII WLAN memory pointer ──────────────────────────────────────────────────
//
// Any pointer returned by a WlanXxx function must be freed with
// WlanFreeMemory.  This wrapper guarantees that even on early return.

#[cfg(target_os = "windows")]
struct WlanPtr<T>(*mut T);

#[cfg(target_os = "windows")]
impl<T> Drop for WlanPtr<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WlanFreeMemory(self.0 as *const _) };
        }
    }
}

#[cfg(target_os = "windows")]
impl<T> WlanPtr<T> {
    fn null() -> Self {
        Self(std::ptr::null_mut())
    }
    fn as_mut_ptr(&mut self) -> *mut *mut T {
        &mut self.0
    }
    unsafe fn as_ref(&self) -> Option<&T> {
        if self.0.is_null() {
            None
        } else {
            Some(&*self.0)
        }
    }
}

// ── Registry helper: friendly name for a WLAN interface GUID ─────────────────

/// Read the user-visible adapter name from the registry.
///
/// Path: HKLM\SYSTEM\CurrentControlSet\Control\Network\
///         {4D36E972-E325-11CE-BFC1-08002BE10318}\<GUID>\Connection
/// Value: "Name"  (REG_SZ)
///
/// Falls back to an empty string if the key is absent (the description
/// will still be used for matching in that case).
#[cfg(target_os = "windows")]
fn registry_friendly_name(guid: &GUID) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
        REG_VALUE_TYPE,
    };

    // Format GUID as {xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}
    let guid_str = format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    );

    let subkey = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\\
         {{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{}\\Connection",
        guid_str
    );

    // Encode as null-terminated UTF-16
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = "Name\0".encode_utf16().collect();

    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let rc = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_w.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );
        if rc.is_err() {
            return String::new();
        }

        let mut data_type = REG_VALUE_TYPE(0);
        let mut data_size = 0u32;

        // First call: get required buffer size
        let _ = RegQueryValueExW(
            hkey,
            PCWSTR(value_w.as_ptr()),
            None,
            Some(std::ptr::addr_of_mut!(data_type).cast()),
            None,
            Some(&mut data_size),
        );

        let name = if data_type == REG_SZ && data_size > 0 {
            let mut buf = vec![0u8; data_size as usize];
            let rc2 = RegQueryValueExW(
                hkey,
                PCWSTR(value_w.as_ptr()),
                None,
                Some(std::ptr::addr_of_mut!(data_type).cast()),
                Some(buf.as_mut_ptr()),
                Some(&mut data_size),
            );
            if rc2.is_ok() {
                // buf is raw bytes of UTF-16; reinterpret as u16 slice
                let u16_len = data_size as usize / 2;
                let u16_slice = std::slice::from_raw_parts(buf.as_ptr() as *const u16, u16_len);
                // Strip null terminator(s)
                let trimmed: Vec<u16> =
                    u16_slice.iter().take_while(|&&c| c != 0).copied().collect();
                OsString::from_wide(&trimmed).to_string_lossy().into_owned()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let _ = RegCloseKey(hkey);
        name
    }
}

// ── Interface enumeration ─────────────────────────────────────────────────────

/// Returns friendly names for all WLAN interfaces (e.g. "Wi-Fi", "Wi-Fi 3").
/// Falls back to description if the registry lookup fails.
#[cfg(target_os = "windows")]
fn enumerate_interfaces(handle: HANDLE) -> Result<Vec<IfaceInfo>> {
    let mut list: WlanPtr<WLAN_INTERFACE_INFO_LIST> = WlanPtr::null();
    let rc = unsafe { WlanEnumInterfaces(handle, None, list.as_mut_ptr()) };
    if rc != ERROR_SUCCESS.0 {
        return Err(
            WifimonError::Platform(format!("WlanEnumInterfaces failed: error {rc}")).into(),
        );
    }

    let iface_list = unsafe {
        list.as_ref()
            .ok_or_else(|| WifimonError::Platform("null interface list".into()))?
    };

    let count = iface_list.dwNumberOfItems as usize;
    let infos = unsafe { std::slice::from_raw_parts(iface_list.InterfaceInfo.as_ptr(), count) };

    let result: Vec<IfaceInfo> = infos
        .iter()
        .map(|info| {
            let desc: Vec<u16> = info
                .strInterfaceDescription
                .iter()
                .take_while(|&&c| c != 0)
                .copied()
                .collect();
            let description = String::from_utf16_lossy(&desc).to_string();

            let friendly = registry_friendly_name(&info.InterfaceGuid);
            // If registry lookup failed, show the description as the name
            let friendly = if friendly.is_empty() {
                description.clone()
            } else {
                friendly
            };

            IfaceInfo {
                guid: info.InterfaceGuid,
                friendly,
                description,
            }
        })
        .collect();

    Ok(result)
}

#[cfg(target_os = "windows")]
fn interfaces_sync() -> Result<Vec<String>> {
    let session = WlanSession::open()?;
    let ifaces = enumerate_interfaces(session.raw())?;
    // Return friendly names ("Wi-Fi", "Wi-Fi 3") for display / -i matching
    let names: Vec<String> = ifaces.into_iter().map(|i| i.friendly).collect();
    debug!(count = names.len(), "Enumerated WLAN interfaces");
    Ok(names)
}

#[cfg(not(target_os = "windows"))]
fn interfaces_sync() -> Result<Vec<String>> {
    Err(WifimonError::Platform("Windows backend called on non-Windows OS".into()).into())
}

// ── BSS scan ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn scan_sync(pattern: &str) -> Result<Vec<AccessPoint>> {
    let session = WlanSession::open()?;
    let ifaces = enumerate_interfaces(session.raw())?;

    // Find all interfaces matching the pattern
    let matched: Vec<&IfaceInfo> = ifaces.iter().filter(|i| i.matches(pattern)).collect();

    if matched.is_empty() {
        // Give a helpful error listing what IS available
        let available: Vec<String> = ifaces
            .iter()
            .map(|i| format!("\"{}\" ({})", i.friendly, i.description))
            .collect();
        return Err(WifimonError::ScanFailed {
            iface: pattern.to_string(),
            reason: format!(
                "No interface matched pattern '{pattern}'. Available: {}",
                available.join(", ")
            ),
        }
        .into());
    }

    let mut all_aps = Vec::new();
    for iface in matched {
        match scan_one(session.raw(), iface) {
            Ok(mut aps) => all_aps.append(&mut aps),
            Err(e) => tracing::warn!(
                iface = %iface.friendly,
                error = %e,
                "Scan failed on interface"
            ),
        }
    }
    Ok(all_aps)
}

#[cfg(not(target_os = "windows"))]
fn scan_sync(_pattern: &str) -> Result<Vec<AccessPoint>> {
    Err(WifimonError::Platform("Windows backend called on non-Windows OS".into()).into())
}

// ── Scan a single interface by IfaceInfo ──────────────────────────────────────

#[cfg(target_os = "windows")]
fn scan_one(handle: HANDLE, iface: &IfaceInfo) -> Result<Vec<AccessPoint>> {
    // Trigger scan — returns immediately, driver scans in background
    let rc = unsafe { WlanScan(handle, &iface.guid, None, None, None) };
    if rc != ERROR_SUCCESS.0 {
        return Err(WifimonError::Platform(format!("WlanScan failed: error {rc}")).into());
    }

    let bss_list = poll_bss_list(handle, &iface.guid)?;

    let list_ref = unsafe {
        bss_list
            .as_ref()
            .ok_or_else(|| WifimonError::Platform("null BSS list".into()))?
    };

    let count = list_ref.dwNumberOfItems as usize;
    let entries = unsafe { std::slice::from_raw_parts(list_ref.wlanBssEntries.as_ptr(), count) };

    let mut aps = Vec::with_capacity(count);
    for entry in entries {
        let ssid_len = entry.dot11Ssid.uSSIDLength as usize;
        let ssid_bytes = &entry.dot11Ssid.ucSSID[..ssid_len];
        let ssid = String::from_utf8_lossy(ssid_bytes)
            .trim_matches('\0')
            .to_string();

        let bssid = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            entry.dot11Bssid[0],
            entry.dot11Bssid[1],
            entry.dot11Bssid[2],
            entry.dot11Bssid[3],
            entry.dot11Bssid[4],
            entry.dot11Bssid[5],
        );

        // ulChCenterFrequency is in kHz on Windows
        let freq_mhz = entry.ulChCenterFrequency / 1000;
        let channel = freq_to_channel(freq_mhz);

        aps.push(AccessPoint {
            ssid: if ssid.is_empty() { None } else { Some(ssid) },
            bssid,
            signal: entry.lRssi,
            channel: Some(channel),
            frequency: Some(freq_mhz),
            security: vec![Security::Unknown("WPA2".into())],
            interface: iface.friendly.clone(),
            last_seen: Utc::now(),
            connected: false,
        });
    }

    debug!(count = aps.len(), iface = %iface.friendly, "BSS scan complete");
    Ok(aps)
}

// ── Poll WlanGetNetworkBssList until results arrive ───────────────────────────

#[cfg(target_os = "windows")]
fn poll_bss_list(
    handle: HANDLE,
    guid: &GUID,
) -> Result<WlanPtr<windows::Win32::NetworkManagement::WiFi::WLAN_BSS_LIST>> {
    const MAX_WAIT_MS: u64 = 6_000;
    const POLL_MS: u64 = 250;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(MAX_WAIT_MS);

    loop {
        let mut bss_ptr: WlanPtr<windows::Win32::NetworkManagement::WiFi::WLAN_BSS_LIST> =
            WlanPtr::null();

        let rc = unsafe {
            WlanGetNetworkBssList(
                handle,
                guid,
                None,
                dot11_BSS_type_any,
                false,
                None,
                bss_ptr.as_mut_ptr(),
            )
        };

        if rc == ERROR_SUCCESS.0 {
            // Check that at least one entry came back
            if let Some(list) = unsafe { bss_ptr.as_ref() } {
                if list.dwNumberOfItems > 0 {
                    return Ok(bss_ptr);
                }
            }
        }

        if std::time::Instant::now() >= deadline {
            // Return whatever we have (possibly empty)
            return Ok(bss_ptr);
        }

        std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
    }
}

// ── Frequency → channel ───────────────────────────────────────────────────────

fn freq_to_channel(mhz: u32) -> u32 {
    match mhz {
        2412..=2472 => (mhz - 2407) / 5,
        2484 => 14,
        5180..=5885 => (mhz - 5000) / 5,
        _ => 0,
    }
}
