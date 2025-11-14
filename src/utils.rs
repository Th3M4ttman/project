use serde_json::{json, Value};
use std::path::Path;
use std::fs;

pub fn read_json(path: &Path) -> Value {
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    }
}
