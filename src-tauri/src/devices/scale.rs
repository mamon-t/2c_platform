// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Весы (RS-232 / USB-COM).
//!
//! Протокол описывается пользователем в настройках устройства:
//!   settings.pattern — regex с ОДНОЙ capture-группой = вес числом
//!   settings.unit    — "kg" | "g"  (множитель в граммы)
//!
//! Пример CAS ER Plus:  строка "ST,NT,+   1.234 kg"
//!   pattern: `^ST,.*?([0-9]+\.?[0-9]*)\s*kg`
//!   unit: "kg"
//!
//! Стабильность: два одинаковых показания подряд → stable=true.

use regex::Regex;
use tokio::io::AsyncReadExt;
use tokio_serial::SerialPortBuilderExt;
use tokio::sync::{mpsc, watch};

use super::{DeviceDriver, DeviceEvent};

pub struct SerialScale {
    port: String,
    baud: u32,
    pattern: Regex,
    /// Множитель в граммы (kg → 1000.0, g → 1.0)
    unit_mul: f64,
}

impl SerialScale {
    pub fn new(port: String, baud: u32, pattern: Option<&str>, unit: Option<&str>) -> Result<Self, String> {
        let pat = pattern.unwrap_or("^.*?([0-9]+\\.?[0-9]*)\\s*(kg|г|g)");
        let re = Regex::new(pat).map_err(|e| format!("Невалидный pattern '{pat}': {e}"))?;
        let mul = match unit.unwrap_or("kg") {
            "g" => 1.0,
            _ => 1000.0,
        };
        Ok(Self { port, baud, pattern: re, unit_mul: mul })
    }
}

#[async_trait::async_trait]
impl DeviceDriver for SerialScale {
    async fn start(
        &self,
        tx: mpsc::Sender<DeviceEvent>,
        stop_rx: watch::Receiver<bool>,
    ) -> Result<(), String> {
        let open_result = {
            let builder = tokio_serial::new(&self.port, self.baud);
            let handle = tokio::runtime::Handle::current();
            tokio::task::spawn_blocking(move || handle.block_on(async { builder.open_native_async() }))
                .await
                .map_err(|e| format!("join: {e}"))?
        };

        let mut stream = match open_result {
            Ok(s) => s,
            Err(e) => return Err(super::scanner_friendly_error(&self.port, &e)),
        };

        let device_id = self.port.clone();
        let pattern = self.pattern.clone();
        let unit_mul = self.unit_mul;

        tokio::spawn(async move {
            let mut buf = [0u8; 256];
            let mut line: Vec<u8> = Vec::with_capacity(64);
            let mut last_grams: u64 = 0;
            let mut same_count: u32 = 0;
            let mut stop = stop_rx;

            loop {
                tokio::select! {
                    read = stream.read(&mut buf) => {
                        match read {
                            Ok(0) => break,
                            Ok(n) => {
                                for &b in &buf[..n] {
                                    if b == b'\r' || b == b'\n' {
                                        if line.is_empty() { continue; }
                                        let s = String::from_utf8_lossy(&line).trim().to_string();
                                        line.clear();

                                        if let Some(caps) = pattern.captures(&s) {
                                            if let Some(m) = caps.get(1).and_then(|m| m.as_str().replace(',', ".").parse::<f64>().ok()) {
                                                let grams = (m * unit_mul).round() as u64;
                                                if grams > 0 {
                                                    same_count = if grams == last_grams { same_count + 1 } else { 0 };
                                                    last_grams = grams;
                                                    let _ = tx.send(DeviceEvent::Weighed {
                                                        device_id: device_id.clone(),
                                                        grams,
                                                        stable: same_count >= 1,
                                                    }).await;
                                                }
                                            }
                                        }
                                    } else {
                                        line.push(b);
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(DeviceEvent::Error {
                                    device_id: device_id.clone(),
                                    message: super::scanner_friendly_error(&device_id, &e),
                                }).await;
                                break;
                            }
                        }
                    }
                    _ = stop.changed() => {
                        if *stop.borrow() { break; }
                    }
                }
            }

            let _ = tx.send(DeviceEvent::Disconnected { device_id }).await;
        });

        Ok(())
    }

    async fn test(&self) -> Result<String, String> {
        let builder = tokio_serial::new(&self.port, self.baud).timeout(std::time::Duration::from_millis(500));
        let port_name = self.port.clone();
        let res = tokio::task::spawn_blocking(move || builder.open())
            .await
            .map_err(|e| format!("join: {e}"))?;

        match res {
            Ok(mut p) => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                let mut buf = [0u8; 128];
                let mut partial: Vec<u8> = Vec::new();
                while std::time::Instant::now() < deadline {
                    match p.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            partial.extend_from_slice(&buf[..n]);
                            let text = String::from_utf8_lossy(&partial);
                            if let Some(caps) = self.pattern.captures(&text) {
                                if let Some(m) = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
                                    let kg = m * self.unit_mul / 1000.0;
                                    return Ok(format!("{port_name}: весы отвечают — {:.3} кг", kg));
                                }
                            }
                            if text.len() > 512 { partial.clear(); }
                        }
                        _ => std::thread::sleep(std::time::Duration::from_millis(100)),
                    }
                }
                Ok(format!(
                    "{port_name}: порт открыт за 3с данных по шаблону нет — \
                     поставьте товар на весы или проверьте pattern в настройках"
                ))
            }
            Err(e) => Err(super::scanner_friendly_error(&self.port, &e)),
        }
    }
}
