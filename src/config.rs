use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "lazyTrino", about = "TUI Trino table browser")]
pub struct CliArgs {
    #[arg(long)]
    pub url: Option<String>,

    #[arg(long)]
    pub user: Option<String>,

    #[arg(long, alias = "pass")]
    pub password: Option<String>,
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

