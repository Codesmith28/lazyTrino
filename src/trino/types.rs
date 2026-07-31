use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TrinoResponse {
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
}

#[derive(Debug, Deserialize)]
pub struct TrinoError {
    #[serde(default)]
    pub message: Option<String>,
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
