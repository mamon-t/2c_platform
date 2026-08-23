use serde::{Deserialize, Serialize};

use cpcsp::cert_store::CertStore;
use cpcsp::sign::{self, Signer};
use cpcsp::cpcsp_ffi_linux::raw_constants::*;

// ── DTO для фронтенда ─────────────────────────────────────

/// Информация о сертификате (сериализуемая)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub subject_name: String,
    pub issuer_name: String,
    pub sha1_hash: String,
    pub has_private_key: bool,
    pub is_valid: bool,
}

/// Результат подписи
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureResult {
    /// DER-кодированное подписи (attached или detached)
    pub signature_der: Vec<u8>,
    pub signer_subject: String,
    pub signer_issuer: String,
    pub signer_sha1: String,
    pub is_detached: bool,
}

/// Результат проверки подписи
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResultDto {
    pub valid: bool,
    pub content: Vec<u8>,
    pub signer_subject: Option<String>,
    pub signer_issuer: Option<String>,
    pub signer_sha1: Option<String>,
    pub message: String,
}

// ── SigningService ────────────────────────────────────────

pub struct SigningService;

impl SigningService {
    /// Открыть системное хранилище MY и собрать информацию о сертификатах.
    pub fn list_certificates() -> Result<Vec<CertificateInfo>, String> {
        let store = CertStore::open_system("MY")
            .map_err(|e| format!("Не удалось открыть хранилище MY: {e}"))?;

        let mut result = Vec::new();
        for cert in store.iter() {
            let subject = cert.subject_name().unwrap_or_default();
            let issuer = cert.issuer_name().unwrap_or_default();
            let sha1 = cert.sha1_hash()
                .map(|h| hex::encode(h))
                .unwrap_or_default();
            let has_key = cert.has_private_key();
            let is_valid = cert.verify_time().map(|v| v == 0).unwrap_or(false);

            result.push(CertificateInfo {
                subject_name: subject,
                issuer_name: issuer,
                sha1_hash: sha1,
                has_private_key: has_key,
                is_valid,
            });
        }

        Ok(result)
    }

    /// Подписать данные CMS-сообщением.
    ///
    /// * `data` — данные для подписи
    /// * `cert_sha1` — SHA1 хеш сертификата (hex) для поиска в MY хранилище
    /// * `detached` — отсоединённая подпись
    pub fn sign(
        data: &[u8],
        cert_sha1: &str,
        detached: bool,
    ) -> Result<SignatureResult, String> {
        let store = CertStore::open_system("MY")
            .map_err(|e| format!("Не удалось открыть хранилище MY: {e}"))?;

        let sha1_bytes = hex::decode(cert_sha1)
            .map_err(|e| format!("Невалидный SHA1 hex: {e}"))?;
        if sha1_bytes.len() != 20 {
            return Err("SHA1 должен быть 20 байт".into());
        }

        let cert = store.find_by_sha1(&sha1_bytes)
            .ok_or_else(|| format!("Сертификат с SHA1 {cert_sha1} не найден в MY"))?;

        if !cert.has_private_key() {
            return Err("У сертификата нет приватного ключа".into());
        }

        let subject = cert.subject_name().unwrap_or_default();
        let issuer = cert.issuer_name().unwrap_or_default();
        let sha1_hex = cert.sha1_hash()
            .map(|h| hex::encode(h))
            .unwrap_or_default();

        let signer = Signer::new(&cert, AT_KEYEXCHANGE, szOID_GOST_R3411_2012_256);
        let signature_der = sign::sign_message(&[signer], data, detached)
            .map_err(|e| format!("Ошибка подписи: {e}"))?;

        Ok(SignatureResult {
            signature_der,
            signer_subject: subject,
            signer_issuer: issuer,
            signer_sha1: sha1_hex,
            is_detached: detached,
        })
    }

    /// Проверить подпись CMS-сообщения.
    pub fn verify(signed_blob: &[u8]) -> Result<VerifyResultDto, String> {
        let result = sign::verify_signature(signed_blob)
            .map_err(|e| format!("Ошибка проверки подписи: {e}"))?;

        let signer_subject = result.signer_cert.as_ref()
            .and_then(|c| c.subject_name());
        let signer_issuer = result.signer_cert.as_ref()
            .and_then(|c| c.issuer_name());
        let signer_sha1 = result.signer_cert.as_ref()
            .and_then(|c| c.sha1_hash())
            .map(|h| hex::encode(h));

        Ok(VerifyResultDto {
            valid: true,
            content: result.content,
            signer_subject,
            signer_issuer,
            signer_sha1,
            message: "Подпись действительна".into(),
        })
    }

    /// Проверить отсоединённую (detached) подпись.
    pub fn verify_detached(
        signature_blob: &[u8],
        original_data: &[u8],
    ) -> Result<VerifyResultDto, String> {
        let result = sign::verify_detached_signature(signature_blob, original_data)
            .map_err(|e| format!("Ошибка проверки отсоединённой подписи: {e}"))?;

        let signer_subject = result.signer_cert.as_ref()
            .and_then(|c| c.subject_name());
        let signer_issuer = result.signer_cert.as_ref()
            .and_then(|c| c.issuer_name());
        let signer_sha1 = result.signer_cert.as_ref()
            .and_then(|c| c.sha1_hash())
            .map(|h| hex::encode(h));

        Ok(VerifyResultDto {
            valid: true,
            content: result.content,
            signer_subject,
            signer_issuer,
            signer_sha1,
            message: "Отсоединённая подпись действительна".into(),
        })
    }
}
