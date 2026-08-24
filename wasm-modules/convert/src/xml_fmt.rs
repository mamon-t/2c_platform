// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use quick_xml::events::Event;
use quick_xml::Reader;

pub fn parse(data: &[u8]) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut reader = Reader::from_reader(data);

    let mut objects = Vec::new();
    let mut current_obj: Option<serde_json::Map<String, serde_json::Value>> = None;
    let mut current_field: Option<String> = None;
    let mut buf = Vec::new();
    let mut depth = 0u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "object" {
                    current_obj = Some(serde_json::Map::new());
                    depth = 0;
                } else if current_obj.is_some() {
                    current_field = Some(tag.clone());
                    depth += 1;
                }
            }
            Ok(Event::Text(ref e)) => {
                if let (Some(ref mut obj), Some(ref field)) = (&mut current_obj, &current_field) {
                    let text = e.unescape().map_err(|e| anyhow::anyhow!("XML text error: {}", e))?;
                    obj.insert(field.clone(), serde_json::Value::String(text.to_string()));
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "object" {
                    if let Some(obj) = current_obj.take() {
                        objects.push(serde_json::Value::Object(obj));
                    }
                    current_field = None;
                } else if current_obj.is_some() {
                    current_field = None;
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(objects)
}

pub fn export(objects: &[serde_json::Value]) -> anyhow::Result<super::ExportResult> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<objects>\n");

    for obj in objects {
        xml.push_str("  <object>\n");
        if let serde_json::Value::Object(map) = obj {
            for (key, value) in map {
                let escaped = xml_escape(value);
                xml.push_str(&format!("    <{}>{}</{}>\n", key, escaped, key));
            }
        }
        xml.push_str("  </object>\n");
    }

    xml.push_str("</objects>\n");

    Ok(super::ExportResult {
        data: xml.into_bytes(),
        filename: "export.xml".into(),
        content_type: "application/xml".into(),
    })
}

fn xml_escape(val: &serde_json::Value) -> String {
    let s = match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => return String::new(),
        other => other.to_string(),
    };
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
