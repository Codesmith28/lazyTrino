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

