use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "lazyTrino", about = "TUI Trino table browser")]
pub struct CliArgs {
    #[arg(long, default_value = "http://localhost:57574")]
    pub url: String,

    #[arg(long, default_value = "sarthak")]
    pub user: String,

    #[arg(long)]
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
