use crate::config::Config;
use crate::network::{AccessPoint, ScanDiff};
use anyhow::Result;
use chrono::Local;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use colored::*;
use std::io::Write;
use tracing::Level;
use tracing_subscriber::EnvFilter;

pub fn init_tracing(verbose: u8, quiet: bool, log_file: Option<&std::path::Path>) -> Result<()> {
    let level = if quiet {
        Level::ERROR
    } else {
        match verbose {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    if let Some(path) = log_file {
        // Write to both stderr AND a file by using a boxed make_writer
        // that opens the file fresh each call (append mode).
        let path = path.to_path_buf();
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(move || -> Box<dyn Write> {
                // Try to open the log file in append mode.
                // On failure fall back to stderr only.
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    Ok(file) => Box::new(file),
                    Err(_) => Box::new(std::io::stderr()),
                }
            })
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    }

    Ok(())
}

pub fn clap_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::BrightCyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::BrightCyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::BrightGreen.on_default())
        .placeholder(AnsiColor::BrightYellow.on_default())
        .error(AnsiColor::BrightRed.on_default() | Effects::BOLD)
        .valid(AnsiColor::BrightGreen.on_default() | Effects::BOLD)
        .invalid(AnsiColor::BrightRed.on_default() | Effects::BOLD)
}

pub fn print_version() {
    println!(
        "{} {}  —  {}",
        "wifimon".bright_cyan().bold(),
        env!("CARGO_PKG_VERSION").bright_white(),
        "Hadi Cahyadi <cumulus13@gmail.com>".dimmed()
    );
    println!("{}", env!("CARGO_PKG_HOMEPAGE").dimmed());
}

pub fn print_startup(interfaces: &[String], config: &Config) {
    let sep = "─".repeat(60);
    println!("{}", sep.bright_black());
    println!(
        "  {} {}  {}",
        "⦿".bright_cyan(),
        "wifimon".bright_cyan().bold(),
        env!("CARGO_PKG_VERSION").dimmed()
    );
    println!("{}", sep.bright_black());
    println!(
        "  {} {}",
        "Interfaces:".dimmed(),
        interfaces.join(", ").bright_white()
    );
    println!(
        "  {} {}s",
        "Interval:  ".dimmed(),
        config.interval_secs.to_string().bright_white()
    );
    println!(
        "  {} {}:{}",
        "Growl:     ".dimmed(),
        config.growl_host.bright_white(),
        config.growl_port.to_string().bright_white()
    );
    if let Some(icon) = &config.icon {
        println!(
            "  {} {}",
            "Icon:      ".dimmed(),
            icon.display().to_string().bright_white()
        );
    }
    println!("{}", sep.bright_black());
    println!(
        "  {} Press {} to quit\n",
        "✦".yellow(),
        "Ctrl+C".bright_white().bold()
    );
}

pub fn print_scan(aps: &[AccessPoint], diff: &ScanDiff, _config: &Config) {
    let ts = format!("[{}]", Local::now().format("%H:%M:%S"))
        .bright_black()
        .to_string();

    if !diff.any_change {
        println!(
            "{} {} {} networks, no changes",
            ts,
            "·".dimmed(),
            aps.len().to_string().bright_white()
        );
        return;
    }

    println!(
        "{} {} {}",
        ts,
        "⚡".yellow(),
        "Changes detected:".yellow().bold()
    );

    for ap in &diff.new_aps {
        println!(
            "{}  {} {} {}  ({} dBm, {})",
            ts,
            "+".bright_green().bold(),
            "NEW".bright_green().bold(),
            ap.display_name().bright_white().bold(),
            ap.signal,
            ap.signal_label()
        );
    }
    for ap in &diff.lost_aps {
        println!(
            "{}  {} {} {}",
            ts,
            "−".bright_red().bold(),
            "LOST".bright_red().bold(),
            ap.display_name().white()
        );
    }
    for ch in &diff.signal_changes {
        let dir = if ch.delta > 0 {
            ch.direction().bright_green()
        } else {
            ch.direction().bright_red()
        };
        println!(
            "{}  {} {} {}  {} → {} dBm  (Δ{:+})",
            ts,
            dir,
            "SIG".bright_yellow(),
            ch.ap.display_name().white(),
            ch.old_signal.to_string().dimmed(),
            ch.ap.signal.to_string().bright_white(),
            ch.delta
        );
    }
}

pub fn print_scan_table(aps: &[AccessPoint], iface: &str) {
    println!(
        "\n{} {} ({})\n",
        "⦿".bright_cyan(),
        iface.bright_cyan().bold(),
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    if aps.is_empty() {
        println!("  No networks found.\n");
        return;
    }

    println!(
        "  {:<32} {:<20} {:>7}  {:<5}  {:<8}  {}",
        "SSID".dimmed(),
        "BSSID".dimmed(),
        "SIGNAL".dimmed(),
        "CH".dimmed(),
        "QUALITY".dimmed(),
        "SECURITY".dimmed()
    );
    println!("  {}", "─".repeat(84).bright_black());

    let mut sorted: Vec<&AccessPoint> = aps.iter().collect();
    sorted.sort_by(|a, b| b.signal.cmp(&a.signal));

    for ap in sorted {
        let name = ap.display_name();
        let name_display = if name.len() > 31 {
            format!("{}…", &name[..30])
        } else {
            name.clone()
        };
        let sig_str = format!("{:>4} dBm", ap.signal);
        let sig_col = sig_color(ap.signal, &sig_str);
        let quality = format!("{:>3}%", ap.signal_quality());
        let ch = ap
            .channel
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into());
        let sec = ap
            .security
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("+");
        let name_col = if ap.connected {
            format!("● {name_display}").bright_cyan().bold().to_string()
        } else {
            format!("  {name_display}")
        };

        println!(
            "  {:<32} {:<20} {}  {:<5}  {:<8}  {}",
            name_col,
            ap.bssid.dimmed(),
            sig_col,
            ch,
            quality,
            sec.bright_yellow()
        );
    }
    println!();
}

fn sig_color(signal: i32, text: &str) -> String {
    match signal {
        s if s >= -50 => text.bright_green().to_string(),
        s if s >= -60 => text.green().to_string(),
        s if s >= -70 => text.yellow().to_string(),
        s if s >= -80 => text.bright_red().to_string(),
        _ => text.red().to_string(),
    }
}

pub fn print_json(aps: &[AccessPoint]) {
    match serde_json::to_string_pretty(aps) {
        Ok(j) => println!("{j}"),
        Err(e) => eprintln!("JSON error: {e}"),
    }
}
