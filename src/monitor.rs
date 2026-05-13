//! File: src\monitor.rs
//! Author: Hadi Cahyadi <cumulus13@gmail.com>
//! Date: 2026-05-12
//! Description:
//! License: MIT

use crate::config::Config;
use crate::network::{AccessPoint, ScanDiff, SignalChange};
use crate::notify::Notifier;
use crate::platform;
use crate::state::ScanState;
use crate::ui;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::signal;
use tokio::time;
use tracing::{debug, error, info, warn};

pub struct Monitor {
    config: Config,
    notifier: Notifier,
    state: ScanState,
}

impl Monitor {
    pub async fn new(config: Config) -> Result<Self> {
        let notifier = Notifier::new(&config);
        let state = ScanState::new(config.state_file.clone())?;

        match notifier.register() {
            Ok(_) => info!(host = %config.growl_host, "Registered with Growl"),
            Err(e) => warn!(error = %e, "Growl registration failed (will retry per-notification)"),
        }

        Ok(Self {
            config,
            notifier,
            state,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Resolve interfaces: expand patterns against real interfaces
        let interfaces = self.resolve_interfaces().await?;

        if interfaces.is_empty() {
            return Err(crate::error::WifimonError::NoInterfaces.into());
        }

        ui::print_startup(&interfaces, &self.config);

        let mut ticker = time::interval(Duration::from_secs(self.config.interval_secs));
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let shutdown = Self::shutdown_signal();
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown => { info!("Shutdown signal received"); break; }
                _ = ticker.tick() => { self.tick(&interfaces).await; }
            }
        }

        self.state.save_if_needed()?;
        info!("wifimon stopped");
        Ok(())
    }

    /// Expand user-supplied patterns (or detect all) into concrete interface names.
    async fn resolve_interfaces(&self) -> Result<Vec<String>> {
        if self.config.interfaces.is_empty() {
            // Auto-detect all wireless interfaces
            return platform::detect_wireless_interfaces().await;
        }

        let available = platform::detect_wireless_interfaces().await?;

        let mut resolved = Vec::new();
        for pattern in &self.config.interfaces {
            let matched: Vec<String> = available
                .iter()
                .filter(|name| platform::iface_matches(name, pattern))
                .cloned()
                .collect();

            if matched.is_empty() {
                warn!(
                    pattern,
                    available = ?available,
                    "No interface matched pattern — skipping"
                );
            } else {
                for m in matched {
                    if !resolved.contains(&m) {
                        resolved.push(m);
                    }
                }
            }
        }

        Ok(resolved)
    }

    async fn tick(&mut self, interfaces: &[String]) {
        let mut all_aps: Vec<AccessPoint> = Vec::new();

        for iface in interfaces {
            match platform::scan(iface).await {
                Ok(aps) => {
                    debug!(iface, count = aps.len(), "Scan complete");
                    all_aps.extend(aps);
                }
                Err(e) => error!(iface, error = %e, "Scan failed"),
            }
        }

        let diff = self.compute_diff(&all_aps);

        if self.config.json {
            ui::print_json(&all_aps);
        } else {
            ui::print_scan(&all_aps, &diff, &self.config);
        }

        if diff.any_change {
            if let Err(e) = self.notifier.notify_diff(&diff, &self.config) {
                warn!(error = %e, "Notification error");
            }
        }

        if let Some(ref mut p) = self.state.persistent {
            for ap in &all_aps {
                p.update_ap(ap);
            }
            p.last_scan = Some(chrono::Utc::now());
        }

        self.state.current = all_aps
            .into_iter()
            .map(|ap| (ap.bssid.clone(), ap))
            .collect();

        if let Err(e) = self.state.save_if_needed() {
            warn!(error = %e, "State save failed");
        }
    }

    fn compute_diff(&self, current: &[AccessPoint]) -> ScanDiff {
        let mut diff = ScanDiff::default();

        let cur_map: HashMap<&str, &AccessPoint> =
            current.iter().map(|ap| (ap.bssid.as_str(), ap)).collect();

        for ap in current {
            if !self.state.current.contains_key(&ap.bssid) {
                diff.new_aps.push(ap.clone());
                diff.any_change = true;
            } else {
                let old = &self.state.current[&ap.bssid];
                let delta = ap.signal - old.signal;
                if delta.abs() >= self.config.signal_threshold {
                    diff.signal_changes.push(SignalChange {
                        ap: ap.clone(),
                        old_signal: old.signal,
                        delta,
                    });
                    diff.any_change = true;
                }
            }
        }

        for (bssid, ap) in &self.state.current {
            if !cur_map.contains_key(bssid.as_str()) {
                diff.lost_aps.push(ap.clone());
                diff.any_change = true;
            }
        }

        diff
    }

    async fn shutdown_signal() {
        let ctrl_c = async { signal::ctrl_c().await.expect("Ctrl-C handler failed") };

        #[cfg(unix)]
        let term = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler failed")
                .recv()
                .await;
        };
        #[cfg(not(unix))]
        let term = std::future::pending::<()>();

        tokio::select! { _ = ctrl_c => {} _ = term => {} }
    }
}
