use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub fn read_json(path: &Path) -> Value {
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    }
}
