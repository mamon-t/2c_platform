use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;

use super::service::{SigningService, SignatureResult, VerifyResultDto, CertificateInfo};

// ── Input types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SignDocumentInput {
    /// Данные (Base64)
    pub data_base64: String,
    /// SHA1 хеш сертификата (hex)
    pub cert_sha1: String,
    /// Отсоединённая подпись
    pub detached: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct VerifySignatureInput {
    /// Подпись (Base64)
    pub signature_base64: String,
    /// Исходные данные (Base64, для detached)
    pub data_base64: Option<String>,
}

// ── Commands ──────────────────────────────────────────────

/// Получить список сертификатов из системного хранилища MY.
#[tauri::command]
pub async fn list_crypto_certificates(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CertificateInfo>, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("documents.read").map_err(|e| e.to_string())?;

    tokio::task::spawn_blocking(|| SigningService::list_certificates())
        .await
        .map_err(|e| format!("Ошибка блокирующего вызова: {e}"))?
}

/// Подписать документ (CMS-подпись).
#[tauri::command]
pub async fn sign_document(
    input: SignDocumentInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<SignatureResult, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("documents.approve").map_err(|e| e.to_string())?;

    let data = base64_decode(&input.data_base64)?;
    let detached = input.detached.unwrap_or(false);
    let cert_sha1 = input.cert_sha1.clone();

    let result = tokio::task::spawn_blocking(move || {
        SigningService::sign(&data, &cert_sha1, detached)
    })
    .await
    .map_err(|e| format!("Ошибка блокирующего вызова: {e}"))?;

    result
}

/// Проверить подпись документа.
#[tauri::command]
pub async fn verify_document_signature(
    input: VerifySignatureInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<VerifyResultDto, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("documents.read").map_err(|e| e.to_string())?;

    let signature = base64_decode(&input.signature_base64)?;

    let result = if let Some(data_b64) = input.data_base64 {
        let data = base64_decode(&data_b64)?;
        tokio::task::spawn_blocking(move || {
            SigningService::verify_detached(&signature, &data)
        })
        .await
        .map_err(|e| format!("Ошибка блокирующего вызова: {e}"))?
    } else {
        tokio::task::spawn_blocking(move || {
            SigningService::verify(&signature)
        })
        .await
        .map_err(|e| format!("Ошибка блокирующего вызова: {e}"))?
    };

    result
}

// ── Helpers ───────────────────────────────────────────────

/// Простой Base64 decode (без внешних зависимостей — используем base64 из стандартной библиотеки)
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("Невалидный Base64: {e}"))
}
