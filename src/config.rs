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

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:57574".to_string(),
            user: "sarthak".to_string(),
            password: String::new(),
        }
    }
}

