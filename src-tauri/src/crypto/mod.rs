// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use crate::core::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CryptoProviderKind {
    CryptoproCsp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoProvider {
    pub _id: Id,
    pub kind: CryptoProviderKind,
    pub name: String,
    pub executable_path: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCertificate {
    pub _id: Id,
    pub user_id: UserId,
    pub provider_id: Id,
    pub thumbprint: String,
    pub subject: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: DateTime<Utc>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSignature {
    pub _id: Id,
    pub object_id: Id,
    pub version: i64,
    pub certificate_id: Id,
    pub signer_user_id: UserId,
    pub algorithm: String,
    pub signature_value: String,
    pub signed_at: DateTime<Utc>,
}

pub struct CryptoService;

impl CryptoService {
    pub fn new() -> Self {
        Self
    }

    pub fn provider_name(&self, kind: &CryptoProviderKind) -> &str {
        match kind {
            CryptoProviderKind::CryptoproCsp => "КриптоПро CSP",
        }
    }
}
