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

use reqwest::StatusCode;
use reqwest::header::InvalidHeaderValue;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrinoClientError {
    #[error("failed to create Trino HTTP client: {source}")]
    HttpClientBuild {
        #[source]
        source: reqwest::Error,
    },
    #[error("invalid X-Trino-User header value for user {user:?}: {source}")]
    InvalidUserHeader {
        user: String,
        #[source]
        source: InvalidHeaderValue,
    },
    #[error("failed to send Trino {stage} request: {source}")]
    RequestFailed {
        stage: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("failed to read Trino {stage} response body: {source}")]
    ResponseBodyReadFailed {
        stage: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("Trino returned HTTP {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("failed to parse Trino {stage} response JSON: {source}")]
    ResponseParseFailed {
        stage: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("Trino query failed: {message}")]
    QueryError { message: String },
}
