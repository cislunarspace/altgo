#[allow(dead_code)]
mod audio;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod output;
#[allow(dead_code)]
mod polisher;
#[allow(dead_code)]
mod state_machine;
#[allow(dead_code)]
mod transcriber;

use clap::Parser;

#[derive(Parser)]
#[command(name = "altgo", about = "无需打字，言出法随 — Linux 语音转文字工具")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Print version
    #[arg(short = 'V', long)]
    version: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("altgo {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config_path = cli
        .config
        .map(std::path::PathBuf::from)
        .unwrap_or_else(config::Config::default_config_path);

    let cfg = config::Config::load(&config_path)?;

    // Initialize logging.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cfg.logging.level));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    tracing::info!("altgo starting");

    // TODO: wire up remaining modules in later phases.
    tracing::info!("altgo initialized — waiting for right Alt key");

    // Placeholder: wait for Ctrl-C.
    tokio::signal::ctrl_c().await?;

    tracing::info!("altgo stopped");
    Ok(())
}
