//! File: src\config.rs
//! Author: Hadi Cahyadi <cumulus13@gmail.com>
//! Date: 2026-05-12
//! Description:
//! License: MIT

use crate::args::Cli;
use crate::error::WifimonError;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub interfaces: Vec<String>,
    pub interval_secs: u64,
    pub growl_host: String,
    pub growl_port: u16,
    pub growl_password: Option<String>,
    pub icon: Option<PathBuf>,
    pub signal_threshold: i32,
    pub notify_new: bool,
    pub notify_lost: bool,
    pub notify_signal: bool,
    pub state_file: Option<PathBuf>,
    pub json: bool,
    pub no_color: bool,
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        if cli.interval == 0 {
            return Err(WifimonError::Config("--interval must be >= 1".into()).into());
        }
        if cli.growl_port == 0 {
            return Err(WifimonError::Config("--growl-port must be > 0".into()).into());
        }

        let icon = cli.icon.clone().or_else(|| {
            std::env::current_exe().ok().and_then(|exe| {
                let p = exe.with_file_name("wifimon.png");
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            })
        });

        Ok(Self {
            interfaces: cli.interface.clone().unwrap_or_default(),
            interval_secs: cli.interval,
            growl_host: cli.growl_host.clone(),
            growl_port: cli.growl_port,
            growl_password: cli.growl_password.clone(),
            icon,
            signal_threshold: cli.signal_threshold,
            notify_new: cli.notify_new,
            notify_lost: cli.notify_lost,
            notify_signal: cli.notify_signal,
            state_file: cli.state_file.clone(),
            json: cli.json,
            no_color: cli.no_color,
        })
    }
}
