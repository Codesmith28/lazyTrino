use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(name = "lazyTrino", about = "TUI Trino table browser")]
pub struct CliArgs {
    /// Trino coordinator REST server URL.
    #[arg(long)]
    pub url: Option<String>,

    /// Trino username.
    #[arg(long)]
    pub user: Option<String>,

    /// Trino password.
    #[arg(long, alias = "pass")]
    pub password: Option<String>,

    /// Override the default log level when RUST_LOG is not set.
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub log_level: Option<LogLevel>,

    /// Disable file logging.
    #[arg(long)]
    pub no_log: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

pub fn default_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "trino".to_string())
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
            user: default_user(),
            password: String::new(),
        }
    }
}
