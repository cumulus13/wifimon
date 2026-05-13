use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// wifimon — Professional Wi-Fi monitoring with Growl/GNTP notifications.
#[derive(Parser, Debug)]
#[command(
    name    = "wifimon",
    author  = "Hadi Cahyadi <cumulus13@gmail.com>",
    version,
    about,
    long_about = None,
    after_help = "EXAMPLES:\n  wifimon                        Monitor all interfaces\n  wifimon -i wlan0               Monitor wlan0 only\n  wifimon -i wlan0 -n 5          Scan every 5 s\n  wifimon --growl-host 10.0.0.1  Remote Growl\n  wifimon scan -i wlan0          One-shot scan\n  wifimon list                   List wireless interfaces",
    styles = crate::ui::clap_styles(),
)]
pub struct Cli {
    /// Wireless interface(s) to monitor (-i wlan0 -i wlan1 …). Default: all detected.
    #[arg(short = 'i', long = "interface", value_name = "IFACE",
          action = clap::ArgAction::Append)]
    pub interface: Option<Vec<String>>,

    /// Scan interval in seconds [default: 10]
    #[arg(short = 'n', long, value_name = "SECS", default_value = "10")]
    pub interval: u64,

    /// Growl/GNTP host [default: 127.0.0.1]
    #[arg(
        long,
        value_name = "HOST",
        default_value = "127.0.0.1",
        env = "WIFIMON_GROWL_HOST"
    )]
    pub growl_host: String,

    /// Growl/GNTP port [default: 23053]
    #[arg(
        long,
        value_name = "PORT",
        default_value = "23053",
        env = "WIFIMON_GROWL_PORT"
    )]
    pub growl_port: u16,

    /// Growl/GNTP password
    #[arg(long, value_name = "PASS", env = "WIFIMON_GROWL_PASS")]
    pub growl_password: Option<String>,

    /// Path to notification icon PNG (default: wifimon.png next to binary)
    #[arg(long, value_name = "FILE")]
    pub icon: Option<PathBuf>,

    /// Minimum |dBm| change that triggers a signal notification [default: 5]
    #[arg(long, value_name = "DBM", default_value = "5")]
    pub signal_threshold: i32,

    /// Notify when a known network disappears [default: true]
    #[arg(long, default_value = "true")]
    pub notify_lost: bool,

    /// Notify when a new network is discovered [default: true]
    #[arg(long, default_value = "true")]
    pub notify_new: bool,

    /// Notify when signal changes beyond threshold [default: true]
    #[arg(long, default_value = "true")]
    pub notify_signal: bool,

    /// Persist known-network state to this JSON file
    #[arg(long, value_name = "FILE")]
    pub state_file: Option<PathBuf>,

    /// Write log output to this file in addition to stderr
    #[arg(long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,

    /// Increase verbosity: -v = DEBUG, -vv = TRACE
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress all console output except fatal errors
    #[arg(short, long)]
    pub quiet: bool,

    /// Output scan results as JSON (for scripting)
    #[arg(long)]
    pub json: bool,

    /// Disable coloured output (also honoured via NO_COLOR env var)
    #[arg(long, env = "NO_COLOR")]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List available wireless interfaces and exit
    List,
    /// One-shot scan, print results, then exit
    Scan {
        #[arg(short = 'i', long)]
        interface: Option<String>,
    },
    /// Print version information
    Version,
}
