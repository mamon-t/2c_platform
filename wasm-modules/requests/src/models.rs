// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Модели данных модуля «Заявки».
//! Хранятся в KV-хранилище хоста (module_store) как JSON.

use serde::{Deserialize, Serialize};

// ── Маршрут согласования ──────────────────────────────────

/// Тип утверждающего: конкретный пользователь или роль.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApproverType {
    User,
    Role,
}

/// Один этап маршрута.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    /// Порядковый номер (1..N)
    pub step_order: u32,
    pub approver_type: ApproverType,
    /// UUID пользователя или код роли
    pub approver_id: String,
    /// Отображаемое имя (для UI, заполняет админ)
    #[serde(default)]
    pub approver_name: Option<String>,
    /// Таймаут в часах (0 = без таймаута)
    #[serde(default)]
    pub timeout_hours: u32,
    /// Обязательный ли этап (необязательные можно пропустить)
    #[serde(default = "default_true")]
    pub is_required: bool,
}

fn default_true() -> bool { true }

/// Маршрут согласования. Код маршрута уникален в рамках модуля.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRoute {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub steps: Vec<RouteStep>,
    /// Требовать квалифицированную подпись (submit/approve/reject).
    /// Канцтовары — false, закупки на крупную сумму — true.
    #[serde(default)]
    pub requires_signature: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

// ── Согласование (инстанс процесса) ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    InProgress,
    Approved,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Approved,
    Rejected,
    Skipped,
}

/// Состояние одного этапа в активном согласовании.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub step_order: u32,
    pub approver_type: ApproverType,
    pub approver_id: String,
    #[serde(default)]
    pub approver_name: Option<String>,
    pub status: StepStatus,
    /// Мс с эпохи
    #[serde(default)]
    pub decided_at: Option<u64>,
    #[serde(default)]
    pub comment: Option<String>,
    /// DER-подпись в base64 (если требовалась)
    #[serde(default)]
    pub signature_der: Option<String>,
    /// Слепок: каноничная строка, которая была подписана
    #[serde(default)]
    pub signed_payload: Option<String>,
    #[serde(default)]
    pub payload_sha256: Option<String>,
    /// SHA1 сертификата подписанта (из верификации)
    #[serde(default)]
    pub signer_sha1: Option<String>,
    #[serde(default)]
    pub signer_subject: Option<String>,
    /// Подпись прошла серверную верификацию CMS
    #[serde(default)]
    pub verified: bool,
}

/// Активное согласование заявки.
/// Ключ KV = "approval:{request_id}" — одна активная процедура на заявку.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestApproval {
    pub request_id: String,
    pub route_code: String,
    pub route_name: String,
    pub status: ApprovalStatus,
    /// Индекс текущего этапа в steps
    pub current_step: usize,
    pub steps: Vec<StepState>,
    pub initiator_id: String,
    pub initiator_login: String,
    #[serde(default)]
    pub initiator_name: Option<String>,
    /// Подпись инициатора при отправке (base64 DER)
    #[serde(default)]
    pub submit_signature_der: Option<String>,
    /// Слепок данных на момент отправки
    #[serde(default)]
    pub submitted_payload: Option<String>,
    #[serde(default)]
    pub submitted_payload_sha256: Option<String>,
    #[serde(default)]
    pub submitted_signer_sha1: Option<String>,
    /// Подпись инициатора верифицирована хостом
    #[serde(default)]
    pub submit_verified: bool,
    /// Снимок требования подписи из маршрута на момент отправки
    #[serde(default)]
    pub requires_signature: bool,
    pub submitted_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub last_comment: Option<String>,
}

// ── Входные структуры функций ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SubmitInput {
    pub request_id: String,
    pub route_code: String,
    /// base64 DER подписи инициатора
    #[serde(default)]
    pub signature_der: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DecideInput {
    pub request_id: String,
    #[serde(default)]
    pub comment: Option<String>,
    /// base64 DER подписи утверждающего
    pub signature_der: String,
}
