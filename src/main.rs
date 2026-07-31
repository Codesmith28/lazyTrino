mod app;
mod config;
mod trino;
mod tui;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::fmt::writer::MakeWriter;

struct LogFile;

impl<'a> MakeWriter<'a> for LogFile {
    type Writer = std::fs::File;

    fn make_writer(&self) -> Self::Writer {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("lazytrino.log")
            .expect("Failed to open log file")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(LogFile)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = config::CliArgs::parse();
    let auto_connect = args.url.is_some() && args.user.is_some();
    let default_config = config::ConnectionConfig::default();
    let config = config::ConnectionConfig {
        url: args.url.unwrap_or(default_config.url),
        user: args.user.unwrap_or(default_config.user),
        password: args.password.unwrap_or(default_config.password),
    };

    let mut app = app::App::new(config, auto_connect);

    tui::run(&mut app).await
}
