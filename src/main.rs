// Copyright 2026 Sarthak Siddhpura
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
    let config = config::ConnectionConfig {
        url: args.url,
        user: args.user,
        password: args.password.unwrap_or_default(),
    };

    let mut app = app::App::new(config);

    if let app::Screen::Connect(ref mut s) = app.screen {
        s.url = app.config.url.clone();
        s.user = app.config.user.clone();
    }

    tui::run(&mut app).await
}
