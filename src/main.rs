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

use std::{
    fs::OpenOptions,
    io,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::Parser;
use lazy_trino::{app, config, config_file, tui};
use tracing_subscriber::{EnvFilter, fmt::writer::MakeWriter};

const FALLBACK_LOG_FILE: &str = "lazytrino.log";

#[derive(Debug, Clone)]
struct LogWriter {
    path: Option<PathBuf>,
}

impl LogWriter {
    fn new(no_log: bool) -> Self {
        let path = if no_log {
            None
        } else {
            Some(preferred_log_path().unwrap_or_else(fallback_log_path))
        };

        Self { path }
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = Box<dyn io::Write + Send + 'a>;

    fn make_writer(&self) -> Self::Writer {
        let Some(path) = &self.path else {
            return Box::new(io::sink());
        };

        if let Ok(file) = open_log_file(path) {
            return Box::new(file);
        }

        match open_log_file(&fallback_log_path()) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::sink()),
        }
    }
}

fn preferred_log_path() -> Option<PathBuf> {
    let log_dir = dirs::cache_dir()
        .or_else(dirs::data_local_dir)?
        .join("lazytrino");

    std::fs::create_dir_all(&log_dir).ok()?;
    Some(log_dir.join(FALLBACK_LOG_FILE))
}

fn fallback_log_path() -> PathBuf {
    PathBuf::from(FALLBACK_LOG_FILE)
}

fn open_log_file(path: &Path) -> io::Result<std::fs::File> {
    OpenOptions::new().create(true).append(true).open(path)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = config::CliArgs::parse();
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(args.log_level.unwrap_or_default().as_str()));

    tracing_subscriber::fmt()
        .with_writer(LogWriter::new(args.no_log))
        .with_env_filter(env_filter)
        .init();

    let loaded_config = config_file::load();
    let resolved = config_file::resolve_profile(&args, loaded_config.as_ref());

    let mut app = app::App::new(resolved.config, resolved.auto_connect);

    tui::run(&mut app).await
}
