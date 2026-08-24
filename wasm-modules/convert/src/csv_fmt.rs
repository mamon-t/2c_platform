// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use std::collections::HashMap;

pub fn parse(
    data: &[u8],
    mapping: Option<&HashMap<String, String>>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let text = String::from_utf8_lossy(data).to_string();
    let mut reader = csv::Reader::from_reader(text.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| anyhow::anyhow!("CSV header error: {}", e))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let field_map: HashMap<String, String> = if let Some(m) = mapping {
        m.clone()
    } else {
        headers.iter().map(|h| (h.clone(), h.clone())).collect()
    };

    let mut objects = Vec::new();

    for (i, result) in reader.records().enumerate() {
        let record = result.map_err(|e| anyhow::anyhow!("CSV row {} error: {}", i + 1, e))?;
        let mut obj = serde_json::Map::new();

        for (j, header) in headers.iter().enumerate() {
            if let Some(field_code) = field_map.get(header) {
                let value = record.get(j).unwrap_or("");
                obj.insert(
                    field_code.clone(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }

        objects.push(serde_json::Value::Object(obj));
    }

    Ok(objects)
}

pub fn export(objects: &[serde_json::Value]) -> anyhow::Result<super::ExportResult> {
    if objects.is_empty() {
        return Ok(super::ExportResult {
            data: Vec::new(),
            filename: "export.csv".into(),
            content_type: "text/csv".into(),
        });
    }

    let all_keys: Vec<String> = {
        let mut keys = Vec::new();
        for obj in objects {
            if let serde_json::Value::Object(map) = obj {
                for key in map.keys() {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
            }
        }
        keys
    };

    let mut wtr = csv::Writer::from_writer(Vec::new());
    wtr.write_record(&all_keys)
        .map_err(|e| anyhow::anyhow!("CSV write header error: {}", e))?;

    for obj in objects {
        let record: Vec<String> = all_keys
            .iter()
            .map(|key| {
                obj.get(key)
                    .and_then(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Null => Some(String::new()),
                        other => Some(other.to_string()),
                    })
                    .unwrap_or_default()
            })
            .collect();
        wtr.write_record(&record)
            .map_err(|e| anyhow::anyhow!("CSV write row error: {}", e))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| anyhow::anyhow!("CSV flush error: {}", e))?;

    Ok(super::ExportResult {
        data,
        filename: "export.csv".into(),
        content_type: "text/csv".into(),
    })
}
