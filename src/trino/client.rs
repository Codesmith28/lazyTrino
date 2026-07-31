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

use reqwest::header::{HeaderMap, HeaderValue};

use super::error::TrinoClientError;
use super::queries;
use super::types::{QueryResults, TrinoResponse, format_value};

#[derive(Clone, Debug)]
pub struct TrinoClient {
    http: reqwest::Client,
    server_url: String,
    user: String,
}

impl TrinoClient {
    pub fn new(server_url: &str, user: &str) -> Result<Self, TrinoClientError> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|source| TrinoClientError::HttpClientBuild { source })?;

        Ok(Self {
            http,
            server_url: server_url.trim_end_matches('/').to_string(),
            user: user.to_string(),
        })
    }

    fn headers(&self) -> Result<HeaderMap, TrinoClientError> {
        let mut h = HeaderMap::new();
        h.insert(
            "X-Trino-User",
            HeaderValue::from_str(&self.user).map_err(|source| {
                TrinoClientError::InvalidUserHeader {
                    user: self.user.clone(),
                    source,
                }
            })?,
        );
        Ok(h)
    }

    fn rewrite_uri(&self, uri: &str) -> String {
        if let Some(path) = uri.split("/v1/statement/").nth(1) {
            format!("{}/v1/statement/{path}", self.server_url)
        } else {
            uri.to_string()
        }
    }

    async fn parse_response(
        response: reqwest::Response,
        stage: &'static str,
    ) -> Result<TrinoResponse, TrinoClientError> {
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|source| TrinoClientError::ResponseBodyReadFailed { stage, source })?;

        if !status.is_success() {
            return Err(TrinoClientError::HttpStatus {
                status,
                body: format_response_body(&body),
            });
        }

        serde_json::from_slice(&body)
            .map_err(|source| TrinoClientError::ResponseParseFailed { stage, source })
    }

    fn ensure_success(response: &TrinoResponse) -> Result<(), TrinoClientError> {
        if let Some(err) = &response.error {
            return Err(TrinoClientError::QueryError {
                message: err.message.clone().unwrap_or_else(|| "unknown".to_string()),
            });
        }

        Ok(())
    }

    pub async fn execute(&self, sql: &str) -> Result<QueryResults, TrinoClientError> {
        let start = std::time::Instant::now();
        let headers = self.headers()?;

        let resp = self
            .http
            .post(format!("{}/v1/statement", self.server_url))
            .headers(headers.clone())
            .body(sql.to_string())
            .send()
            .await
            .map_err(|source| TrinoClientError::RequestFailed {
                stage: "query",
                source,
            })?;

        let mut data = Self::parse_response(resp, "query").await?;
        Self::ensure_success(&data)?;

        let mut columns = data.columns.clone().unwrap_or_default();
        let mut all_data: Vec<Vec<String>> = Vec::new();

        if let Some(rows) = &data.data {
            for row in rows {
                all_data.push(row.iter().map(format_value).collect());
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
                .map_err(|source| TrinoClientError::RequestFailed {
                    stage: "pagination",
                    source,
                })?;

            data = Self::parse_response(resp, "pagination").await?;
            Self::ensure_success(&data)?;

            if columns.is_empty()
                && let Some(ref cols) = data.columns
            {
                columns = cols.clone();
            }

            if let Some(rows) = &data.data {
                for row in rows {
                    all_data.push(row.iter().map(format_value).collect());
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

    pub async fn fetch_catalogs(&self) -> Result<Vec<String>, TrinoClientError> {
        let results = self.execute(&queries::show_catalogs()).await?;
        Ok(results
            .data
            .into_iter()
            .map(|r| r.into_iter().next().unwrap_or_default())
            .collect())
    }

    pub async fn fetch_schemas(&self, catalog: &str) -> Result<Vec<String>, TrinoClientError> {
        let results = self.execute(&queries::show_schemas(catalog)).await?;
        Ok(results
            .data
            .into_iter()
            .map(|r| r.into_iter().next().unwrap_or_default())
            .collect())
    }

    pub async fn fetch_tables(
        &self,
        catalog: &str,
        schema: &str,
    ) -> Result<Vec<String>, TrinoClientError> {
        let results = self.execute(&queries::show_tables(catalog, schema)).await?;
        Ok(results
            .data
            .into_iter()
            .map(|r| r.into_iter().next().unwrap_or_default())
            .collect())
    }
}

fn format_response_body(body: &[u8]) -> String {
    let body = String::from_utf8_lossy(body);
    let body = body.trim();
    if body.is_empty() {
        return "<empty response body>".to_string();
    }

    const MAX_CHARS: usize = 200;
    let truncated: String = body.chars().take(MAX_CHARS).collect();
    if body.chars().count() > MAX_CHARS {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};
    use reqwest::StatusCode;
    use serde_json::json;

    #[test]
    fn headers_reject_invalid_user_values() {
        let client = TrinoClient::new("https://example.com", "bad\nuser").unwrap();
        let err = client.headers().unwrap_err();

        assert!(matches!(err, TrinoClientError::InvalidUserHeader { .. }));
    }

    #[test]
    fn headers_accept_valid_user_values() {
        let client = TrinoClient::new("https://example.com", "valid-user").unwrap();
        let headers = client.headers().unwrap();

        assert_eq!(headers["X-Trino-User"], "valid-user");
    }

    #[tokio::test]
    async fn execute_collects_paginated_results_and_formats_values() {
        let mut server = Server::new_async().await;
        let post_mock = server
            .mock("POST", "/v1/statement")
            .match_body(Matcher::Exact("SELECT * FROM test_table".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "columns": [
                        { "name": "id" },
                        { "name": "tags" },
                        { "name": "meta" }
                    ],
                    "data": [
                        [1, ["alpha", true], { "k": "v" }]
                    ],
                    "nextUri": "http://ignored.invalid/v1/statement/queued/1"
                })
                .to_string(),
            )
            .create_async()
            .await;
        let get_mock = server
            .mock("GET", "/v1/statement/queued/1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        [null, ["beta"], { "n": 2 }]
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let client = TrinoClient::new(&server.url(), "test_user").unwrap();
        let results = client.execute("SELECT * FROM test_table").await.unwrap();

        post_mock.assert_async().await;
        get_mock.assert_async().await;

        assert_eq!(
            results
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "tags", "meta"]
        );
        assert_eq!(
            results.data,
            vec![
                vec!["1".to_string(), "[alpha, true]".to_string(), "{\"k\":\"v\"}".to_string()],
                vec!["NULL".to_string(), "[beta]".to_string(), "{\"n\":2}".to_string()],
            ]
        );
        assert!(results.duration_ms < 5_000);
    }

    #[tokio::test]
    async fn execute_returns_http_status_error_for_non_success_response() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/statement")
            .with_status(503)
            .with_header("content-type", "text/plain")
            .with_body("coordinator unavailable")
            .create_async()
            .await;

        let client = TrinoClient::new(&server.url(), "test_user").unwrap();
        let err = client.execute("SELECT 1").await.unwrap_err();

        mock.assert_async().await;

        match err {
            TrinoClientError::HttpStatus { status, body } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
                assert_eq!(body, "coordinator unavailable");
            }
            other => panic!("expected HttpStatus error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_returns_query_error_for_trino_error_payload() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/statement")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "error": { "message": "column not found" } }).to_string())
            .create_async()
            .await;

        let client = TrinoClient::new(&server.url(), "test_user").unwrap();
        let err = client.execute("SELECT missing").await.unwrap_err();

        mock.assert_async().await;

        match err {
            TrinoClientError::QueryError { message } => {
                assert_eq!(message, "column not found");
            }
            other => panic!("expected QueryError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn execute_returns_parse_error_for_malformed_json() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/statement")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{not-json")
            .create_async()
            .await;

        let client = TrinoClient::new(&server.url(), "test_user").unwrap();
        let err = client.execute("SELECT 1").await.unwrap_err();

        mock.assert_async().await;

        match err {
            TrinoClientError::ResponseParseFailed { stage, .. } => {
                assert_eq!(stage, "query");
            }
            other => panic!("expected ResponseParseFailed error, got {other:?}"),
        }
    }
}
