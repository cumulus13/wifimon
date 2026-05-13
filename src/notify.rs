//! File: src\notify.rs
//! Author: Hadi Cahyadi <cumulus13@gmail.com>
//! Date: 2026-05-12
//! Description:
//! License: MIT

use crate::config::Config;
use crate::network::{AccessPoint, ScanDiff, SignalChange};
use anyhow::Result;
use gntp::{GntpClient, IconMode, NotificationType, NotifyOptions, Resource};
use std::path::PathBuf;
use tracing::{debug, warn};

const NOTIF_NEW: &str = "new-network";
const NOTIF_LOST: &str = "lost-network";
const NOTIF_SIGNAL: &str = "signal-change";

pub struct Notifier {
    host: String,
    port: u16,
    password: Option<String>,
    icon_path: Option<PathBuf>,
}

impl Notifier {
    pub fn new(config: &Config) -> Self {
        Self {
            host: config.growl_host.clone(),
            port: config.growl_port,
            password: config.growl_password.clone(),
            icon_path: config.icon.clone(),
        }
    }

    fn load_icon(&self) -> Option<Resource> {
        let path = self.icon_path.as_ref()?;
        match Resource::from_file(path) {
            Ok(r) => {
                debug!(path = %path.display(), "Loaded icon");
                Some(r)
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Icon load failed");
                None
            }
        }
    }

    /// Build a fresh GntpClient AND register it in one step.
    ///
    /// GNTP is stateless from the client's perspective: every TCP connection
    /// is independent.  The server-side "registered" flag lives only for the
    /// lifetime of that connection.  Because each notify() call opens a new
    /// TCP connection (the gntp crate reconnects per call), we must register
    /// on the same client object before we notify.
    fn registered_client(&self, icon: Option<&Resource>) -> Result<GntpClient> {
        let mut client = GntpClient::new("wifimon")
            .with_host(&self.host)
            .with_port(self.port)
            .with_icon_mode(IconMode::DataUrl);

        if let Some(ref p) = self.password {
            client = client.with_password(p);
        }
        if let Some(r) = icon {
            client = client.with_icon(r.clone());
        }

        let make = |id: &str, name: &str| -> NotificationType {
            let t = NotificationType::new(id).with_display_name(name);
            match icon {
                Some(r) => t.with_icon(r.clone()),
                None => t,
            }
        };

        let types = vec![
            make(NOTIF_NEW, "New Network Discovered"),
            make(NOTIF_LOST, "Network Lost"),
            make(NOTIF_SIGNAL, "Signal Changed"),
        ];

        client
            .register(types)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("GNTP register failed: {e}"))?;

        Ok(client)
    }

    /// Called once at startup — just a connectivity / registration smoke-test.
    pub fn register(&self) -> Result<()> {
        let icon = self.load_icon();
        self.registered_client(icon.as_ref())?;
        debug!(host = %self.host, port = self.port, "Startup registration OK");
        Ok(())
    }

    pub fn notify_diff(&self, diff: &ScanDiff, config: &Config) -> Result<()> {
        if config.notify_new {
            for ap in &diff.new_aps {
                if let Err(e) = self.notify_new(ap) {
                    warn!(bssid = %ap.bssid, error = %e, "new-network notify failed");
                }
            }
        }
        if config.notify_lost {
            for ap in &diff.lost_aps {
                if let Err(e) = self.notify_lost(ap) {
                    warn!(bssid = %ap.bssid, error = %e, "lost-network notify failed");
                }
            }
        }
        if config.notify_signal {
            for ch in &diff.signal_changes {
                if let Err(e) = self.notify_signal(ch) {
                    warn!(error = %e, "signal-change notify failed");
                }
            }
        }
        Ok(())
    }

    fn notify_new(&self, ap: &AccessPoint) -> Result<()> {
        let title = format!("New Wi-Fi: {}", ap.display_name());
        let body = format!(
            "BSSID: {}  |  {} dBm ({})  |  iface: {}{}",
            ap.bssid,
            ap.signal,
            ap.signal_label(),
            ap.interface,
            ap.channel
                .map(|c| format!("  |  Ch {c}"))
                .unwrap_or_default()
        );
        self.send(NOTIF_NEW, &title, &body)
    }

    fn notify_lost(&self, ap: &AccessPoint) -> Result<()> {
        let title = format!("Wi-Fi Lost: {}", ap.display_name());
        let body = format!(
            "BSSID: {}  |  last {} dBm  |  iface: {}",
            ap.bssid, ap.signal, ap.interface
        );
        self.send(NOTIF_LOST, &title, &body)
    }

    fn notify_signal(&self, ch: &SignalChange) -> Result<()> {
        let ap = &ch.ap;
        let verb = if ch.delta > 0 { "Improved" } else { "Degraded" };
        let title = format!("Signal {verb} — {}", ap.display_name());
        let body = format!(
            "BSSID: {}  |  {} {} → {} dBm  (Δ{:+})  |  iface: {}",
            ap.bssid,
            ch.direction(),
            ch.old_signal,
            ap.signal,
            ch.delta,
            ap.interface
        );
        self.send(NOTIF_SIGNAL, &title, &body)
    }

    /// Core send: register + notify on the SAME client instance.
    ///
    /// Each invocation opens a fresh TCP connection to Growl, registers
    /// the app and notification types, then immediately sends the notification.
    /// This is the correct pattern for the gntp crate's connection model.
    fn send(&self, notif_type: &str, title: &str, body: &str) -> Result<()> {
        debug!(notif_type, title, "Sending GNTP notification");

        let icon = self.load_icon();

        // Register and notify on the same client — never reuse across calls
        let client = self.registered_client(icon.as_ref())?;

        let opts = NotifyOptions::new().with_sticky(false).with_priority(0);

        client
            .notify_with_options(notif_type, title, body, opts)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("GNTP notify ({notif_type}): {e}"))
    }
}
