pub fn parse(data: &[u8]) -> anyhow::Result<Vec<serde_json::Value>> {
    let text = String::from_utf8_lossy(data).to_string();
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(&text).map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;

    match parsed {
        serde_yaml::Value::Sequence(seq) => {
            let mut objects = Vec::new();
            for item in seq {
                let json_val = yaml_to_json(item);
                objects.push(json_val);
            }
            Ok(objects)
        }
        serde_yaml::Value::Mapping(_) => {
            let json_val = yaml_to_json(parsed);
            Ok(vec![json_val])
        }
        _ => Err(anyhow::anyhow!("YAML root must be a sequence or mapping")),
    }
}

fn yaml_to_json(val: serde_yaml::Value) -> serde_json::Value {
    match val {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => format!("{:?}", other),
                    };
                    (key, yaml_to_json(v))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

fn json_to_yaml(val: &serde_json::Value) -> serde_yaml::Value {
    match val {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(i))
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::Null
            }
        }
        serde_json::Value::String(s) => serde_yaml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.iter().map(json_to_yaml).collect())
        }
        serde_json::Value::Object(map) => {
            let m: serde_yaml::Mapping = map
                .iter()
                .map(|(k, v)| (serde_yaml::Value::String(k.clone()), json_to_yaml(v)))
                .collect();
            serde_yaml::Value::Mapping(m)
        }
    }
}

pub fn export(objects: &[serde_json::Value]) -> anyhow::Result<super::ExportResult> {
    let yaml_objects: Vec<serde_yaml::Value> = objects.iter().map(json_to_yaml).collect();

    let val = if yaml_objects.len() == 1 {
        yaml_objects.into_iter().next().unwrap()
    } else {
        serde_yaml::Value::Sequence(yaml_objects)
    };

    let data =
        serde_yaml::to_string(&val).map_err(|e| anyhow::anyhow!("YAML serialize error: {}", e))?;

    Ok(super::ExportResult {
        data: data.into_bytes(),
        filename: "export.yaml".into(),
        content_type: "text/yaml".into(),
    })
}
