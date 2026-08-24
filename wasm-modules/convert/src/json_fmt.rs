// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

pub fn parse(data: &[u8]) -> anyhow::Result<Vec<serde_json::Value>> {
    let text = String::from_utf8_lossy(data).to_string();
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;

    match parsed {
        serde_json::Value::Array(arr) => Ok(arr),
        serde_json::Value::Object(_) => Ok(vec![parsed]),
        _ => Err(anyhow::anyhow!("JSON root must be an array or object")),
    }
}

pub fn export(objects: &[serde_json::Value]) -> anyhow::Result<super::ExportResult> {
    let data = serde_json::to_string_pretty(objects)
        .map_err(|e| anyhow::anyhow!("JSON serialize error: {}", e))?;

    Ok(super::ExportResult {
        data: data.into_bytes(),
        filename: "export.json".into(),
        content_type: "application/json".into(),
    })
}
