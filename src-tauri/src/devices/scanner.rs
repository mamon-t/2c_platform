// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Serial-сканер штрихкодов (/dev/ttyUSB*, /dev/ttyACM*).
//!
//! Сканер шлёт код строкой с терминатором \r или \n.
//! Читаем байты, накапливаем буфер, по терминатору — DeviceEvent::Scanned.

use tokio::io::AsyncReadExt;
use tokio_serial::SerialPortBuilderExt;
use tokio::sync::{mpsc, watch};

use super::{DeviceDriver, DeviceEvent};

pub struct SerialScanner {
    port: String,
    baud: u32,
}

impl SerialScanner {
    pub fn new(port: String, baud: u32) -> Self {
        Self { port, baud }
    }
}

/// Человекочитаемая подсказка для частых ошибок Linux.
pub fn scanner_friendly_error(port: &str, e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    if raw.contains("Permission denied") {
        format!(
            "{port}: доступ запрещён. Пользователь не в группе dialout.\n\
             Выполните: sudo usermod -aG dialout $USER — и перелогиньтесь."
        )
    } else if raw.contains("No such file or directory") {
        format!("{port}: порт не найден. Переподключите устройство и обновите список портов.")
    } else if raw.contains("Device or resource busy") {
        format!("{port}: занят другим процессом (например, модемным менеджером).")
    } else {
        format!("{port}: {raw}")
    }
}

#[async_trait::async_trait]
impl DeviceDriver for SerialScanner {
    async fn start(
        &self,
        tx: mpsc::Sender<DeviceEvent>,
        stop_rx: watch::Receiver<bool>,
    ) -> Result<(), String> {
        let port = self.port.clone();
        let baud = self.baud;

        // Открытие порта блокирующее (serialport) → spawn_blocking,
        // дальше асинхронное чтение через tokio_serial::SerialStream.
        let open_result = {
            let port = port.clone();
            let handle = tokio::runtime::Handle::current();
            tokio::task::spawn_blocking(move || {
                handle.block_on(async {
                    tokio_serial::new(&port, baud).open_native_async()
                })
            })
            .await
            .map_err(|e| format!("join: {e}"))?
        };

        let mut stream = match open_result {
            Ok(s) => s,
            Err(e) => {
                let msg = scanner_friendly_error(&port, &e);
                let _ = tx
                    .send(DeviceEvent::Error { device_id: port.clone(), message: msg.clone() })
                    .await;
                return Err(msg);
            }
        };

        let device_id = port.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 256];
            let mut line: Vec<u8> = Vec::with_capacity(64);
            let mut stop = stop_rx;

            loop {
                tokio::select! {
                    read = stream.read(&mut buf) => {
                        match read {
                            Ok(0) => break, // EOF: устройство отключено
                            Ok(n) => {
                                for &b in &buf[..n] {
                                    if b == b'\r' || b == b'\n' {
                                        if !line.is_empty() {
                                            let code = String::from_utf8_lossy(&line)
                                                .trim()
                                                .to_string();
                                            line.clear();
                                            if code.len() >= 4 {
                                                let _ = tx.send(DeviceEvent::Scanned {
                                                    device_id: device_id.clone(),
                                                    code,
                                                }).await;
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
                                    message: scanner_friendly_error(&device_id, &e),
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
        // Открыть порт на 2 секунды: если открылся — порт жив, ждём данные.
        let serialport_builder = tokio_serial::new(&self.port, self.baud).timeout(std::time::Duration::from_millis(500));
        let port_name = self.port.clone();
        let res = tokio::task::spawn_blocking(move || serialport_builder.open())
            .await
            .map_err(|e| format!("join: {e}"))?;

        match res {
            Ok(mut p) => {
                // Пробуем прочитать 2 секунды
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                let mut buf = [0u8; 128];
                while std::time::Instant::now() < deadline {
                    match p.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            return Ok(format!(
                                "{}: порт открыт, данные идут ({} байт) — сканер работает",
                                port_name, n
                            ))
                        }
                        _ => std::thread::sleep(std::time::Duration::from_millis(100)),
                    }
                }
                Ok(format!(
                    "{port_name}: порт открыт, данных нет за 2с — отсканируйте тестовый штрихкод"
                ))
            }
            Err(e) => Err(scanner_friendly_error(&self.port, &e)),
        }
    }
}
