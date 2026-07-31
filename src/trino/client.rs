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

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};

use super::queries;
use super::types::{format_value, QueryResults, TrinoResponse};

#[derive(Clone, Debug)]
pub struct TrinoClient {
    http: reqwest::Client,
    server_url: String,
    user: String,
}

impl TrinoClient {
    pub fn new(server_url: &str, user: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            server_url: server_url.trim_end_matches('/').to_string(),
            user: user.to_string(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            "X-Trino-User",
            HeaderValue::from_str(&self.user).expect("Invalid header value"),
        );
        h
    }

    fn rewrite_uri(&self, uri: &str) -> String {
        if let Some(path) = uri.split("/v1/statement/").nth(1) {
            format!("{}/v1/statement/{path}", self.server_url)
        } else {
            uri.to_string()
        }
    }

    pub async fn execute(&self, sql: &str) -> Result<QueryResults> {
        let start = std::time::Instant::now();
        let headers = self.headers();

        let resp = self
            .http
            .post(format!("{}/v1/statement", self.server_url))
            .headers(headers.clone())
            .body(sql.to_string())
            .send()
            .await
            .context("Failed to send query to Trino")?;

        let mut data: TrinoResponse = resp
            .json()
            .await
            .context("Failed to parse Trino response")?;

        if let Some(ref err) = data.error {
            anyhow::bail!(
                "Trino error: {}",
                err.message.as_deref().unwrap_or("unknown")
            );
        }

        let mut columns = data.columns.clone().unwrap_or_default();
        let mut all_data: Vec<Vec<String>> = Vec::new();

        if let Some(rows) = &data.data {
            for row in rows {
                all_data.push(row.iter().map(|v| format_value(v)).collect());
            }
        }

        while let Some(ref next_uri) = data.next_uri {
            let uri = self.rewrite_uri(next_uri);

            let resp = self
                .http
                .get(&uri)
                .headers(headers.clone())
                .send()
                .await
                .context("Failed to fetch next page from Trino")?;

            data = resp
                .json()
                .await
                .context("Failed to parse Trino pagination response")?;

            if let Some(ref err) = data.error {
                anyhow::bail!(
                    "Trino error: {}",
                    err.message.as_deref().unwrap_or("unknown")
                );
            }

            if columns.is_empty() {
                if let Some(ref cols) = data.columns {
                    columns = cols.clone();
                }
            }

            if let Some(rows) = &data.data {
                for row in rows {
                    all_data.push(row.iter().map(|v| format_value(v)).collect());
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(QueryResults {
            columns,
            data: all_data,
            duration_ms,
        })
    }

    pub async fn fetch_catalogs(&self) -> Result<Vec<String>> {
        let results = self.execute(&queries::show_catalogs()).await?;
        Ok(results.data.into_iter().map(|r| r.into_iter().next().unwrap_or_default()).collect())
    }

    pub async fn fetch_schemas(&self, catalog: &str) -> Result<Vec<String>> {
        let results = self.execute(&queries::show_schemas(catalog)).await?;
        Ok(results.data.into_iter().map(|r| r.into_iter().next().unwrap_or_default()).collect())
    }

    pub async fn fetch_tables(&self, catalog: &str, schema: &str) -> Result<Vec<String>> {
        let results = self.execute(&queries::show_tables(catalog, schema)).await?;
        Ok(results.data.into_iter().map(|r| r.into_iter().next().unwrap_or_default()).collect())
    }
}
