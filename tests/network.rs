use chrono::Utc;
use wifimon::network::{AccessPoint, Security};

fn ap(bssid: &str, signal: i32) -> AccessPoint {
    AccessPoint {
        ssid: Some("TestNet".into()),
        bssid: bssid.into(),
        signal,
        channel: Some(6),
        frequency: Some(2437),
        security: vec![Security::Wpa2],
        interface: "wlan0".into(),
        last_seen: Utc::now(),
        connected: false,
    }
}

#[test]
fn quality_excellent() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -30).signal_quality(), 100);
}
#[test]
fn quality_good() {
    assert!(ap("AA:BB:CC:DD:EE:FF", -55).signal_quality() > 50);
}
#[test]
fn quality_floor() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -100).signal_quality(), 0);
}
#[test]
fn label_excellent() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -45).signal_label(), "Excellent");
}
#[test]
fn label_very_poor() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -85).signal_label(), "Poor");
}
#[test]
fn display_name_ssid() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -60).display_name(), "TestNet");
}
#[test]
fn display_name_hidden() {
    let mut a = ap("AA:BB:CC:DD:EE:FF", -60);
    a.ssid = None;
    assert!(a.display_name().contains("hidden"));
}
#[test]
fn display_name_empty() {
    let mut a = ap("AA:BB:CC:DD:EE:FF", -60);
    a.ssid = Some(String::new());
    assert!(a.display_name().contains("hidden"));
}
#[test]
fn eq_same_bssid() {
    assert_eq!(ap("AA:BB:CC:DD:EE:FF", -60), ap("AA:BB:CC:DD:EE:FF", -70));
}
#[test]
fn ne_diff_iface() {
    let mut b = ap("AA:BB:CC:DD:EE:FF", -60);
    b.interface = "wlan1".into();
    assert_ne!(ap("AA:BB:CC:DD:EE:FF", -60), b);
}
#[test]
fn security_display() {
    assert_eq!(Security::Wpa2.to_string(), "WPA2");
    assert_eq!(Security::Open.to_string(), "Open");
}
