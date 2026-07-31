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
