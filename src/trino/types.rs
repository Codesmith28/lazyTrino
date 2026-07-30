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

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TrinoResponse {
    #[allow(dead_code)]
    #[serde(default)]
    pub id: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "infoUri", default)]
    pub info_uri: Option<String>,
    #[serde(rename = "nextUri", default)]
    pub next_uri: Option<String>,
    #[serde(default)]
    pub columns: Option<Vec<Column>>,
    #[serde(default)]
    pub data: Option<Vec<Vec<Option<serde_json::Value>>>>,
    #[serde(default)]
    pub error: Option<TrinoError>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Column {
    pub name: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Deserialize)]
pub struct TrinoError {
    #[serde(default)]
    pub message: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "errorCode", default)]
    pub error_code: Option<u32>,
    #[allow(dead_code)]
    #[serde(rename = "errorName", default)]
    pub error_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryResults {
    pub columns: Vec<Column>,
    pub data: Vec<Vec<String>>,
    pub duration_ms: u64,
}

pub fn format_value(v: &Option<serde_json::Value>) -> String {
    match v {
        None | Some(serde_json::Value::Null) => "NULL".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(serde_json::Value::Array(arr)) => {
            let items: Vec<String> = arr.iter().map(|v| format_value(&Some(v.clone()))).collect();
            format!("[{}]", items.join(", "))
        }
        Some(serde_json::Value::Object(obj)) => serde_json::to_string(obj).unwrap_or_default(),
    }
}
