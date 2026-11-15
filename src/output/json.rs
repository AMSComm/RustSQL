use super::QueryResult;
use crate::error::Result;
use serde_json::{Map, Value};

pub fn format_json(result: &QueryResult) -> Result<String> {
    let mut array = Vec::new();

    for row in &result.rows {
        let mut obj = Map::new();
        for (idx, col_name) in result.columns.iter().enumerate() {
            let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
            obj.insert(col_name.clone(), Value::String(val.to_string()));
        }
        array.push(Value::Object(obj));
    }

    let json_val = Value::Array(array);
    let s = serde_json::to_string_pretty(&json_val)
        .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;

    Ok(format!("{}\n", s))
}
