use anyhow::Result;
use clap::Parser;
use tracing::info;
use wifimon::{args, args::Cli, config::Config, monitor::Monitor, platform, ui};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    ui::init_tracing(cli.verbose, cli.quiet, cli.log_file.as_deref())?;
    info!(version = env!("CARGO_PKG_VERSION"), "wifimon starting");

    match cli.command {
        Some(args::Commands::List) => {
            platform::list_interfaces().await?;
        }
        Some(args::Commands::Scan { interface }) => {
            let iface = interface.or_else(|| cli.interface.clone()?.into_iter().next());
            platform::scan_once(iface.as_deref()).await?;
        }
        Some(args::Commands::Version) => {
            ui::print_version();
        }
        None => {
            let config = Config::from_cli(&cli)?;
            let mut monitor = Monitor::new(config).await?;
            monitor.run().await?;
        }
    }
    Ok(())
}
