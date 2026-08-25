// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::audit::AuditableAction;
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

// ── Тестовый сертификат (для проверки подписей без УЭЦП) ──

/// Создать самоподписанный сертификат ГОСТ Р 34.10-2012 и установить в MY.
/// Контейнер: 2c_test_<8 hex>. Имя — латиницей (ANSI API КриптоПро).
#[tauri::command]
pub async fn create_test_certificate(
    name: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    // Безопасное извлечение данных ПОД ЛОКОМ (только чтение)
    let db = {
        let state = state.lock().await;
        let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
        ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
        ctx.db.clone()
    };

    // ANSI-безопасное имя
    let safe: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '.')
        .take(40)
        .collect();
    if safe.trim().is_empty() {
        return Err("Имя должно содержать латинские буквы/цифры".into());
    }
    let subject = format!("CN={}, O=2C-Test", safe.trim());
    let container = format!("2c_test_{}", uuid::Uuid::new_v4().simple().to_string()[..8].to_string());
    let container_out = container.clone();
    let subject_out = subject.clone();

    // Тяжёлая крипто-операция БЕЗ ЛОКА
    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        use cpcsp::cpcsp_ffi_linux::raw_constants::*;
        use cpcsp::key::Key;
        use cpcsp::pki::Pki;
        use cpcsp::provider::Provider;
        use cpcsp::selfsign;

        let prov = Provider::acquire(Some(&container), None, PROV_GOST_2012_256, CRYPT_NEWKEYSET)
            .map_err(|e| format!("Контейнер {container}: {e}"))?;

        let _key = Key::gen(prov.raw_handle(), CALG_GOST_2012_256, 0)
            .map_err(|e| format!("Генерация ключа: {e}"))?;

        let cert = selfsign::create_self_signed(
            &prov,
            &subject,
            AT_KEYEXCHANGE,
            szOID_GOST_R3411_2012_256,
            1,
        )
        .map_err(|e| format!("Создание сертификата: {e}"))?;

        let der = cert.to_der().map_err(|e| format!("Кодирование DER: {e}"))?;

        Pki::install_certificate(prov.raw_handle(), AT_KEYEXCHANGE, &der, "MY", 0, true)
            .map_err(|e| format!("Установка в MY: {e}"))?;

        let sha1 = cert.sha1_hash()
            .map(|h| hex::encode(h))
            .unwrap_or_default();
        Ok(sha1)
    })
    .await
    .map_err(|e| format!("Ошибка блокирующего вызова: {e}"))??;

    // Аудит после успешного создания
    {
        let state = state.lock().await;
        crate::audit_log!(state, db, AuditableAction::CreateTestCertificate,
            target_id = container_out.clone());
    }

    Ok(format!("{container_out}|{subject_out}|{result}"))
}
