// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditChanges {
    #[serde(flatten)]
    pub fields: HashMap<String, FieldChange>,
}

impl AuditChanges {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn field(mut self, name: impl Into<String>, old: impl Into<String>, new: impl Into<String>) -> Self {
        self.fields.insert(
            name.into(),
            FieldChange { old: Some(old.into()), new: Some(new.into()) },
        );
        self
    }

    pub fn field_new(mut self, name: impl Into<String>, new: impl Into<String>) -> Self {
        self.fields.insert(
            name.into(),
            FieldChange { old: None, new: Some(new.into()) },
        );
        self
    }

    pub fn field_old(mut self, name: impl Into<String>, old: impl Into<String>) -> Self {
        self.fields.insert(
            name.into(),
            FieldChange { old: Some(old.into()), new: None },
        );
        self
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn field_names(&self) -> Vec<&str> {
        self.fields.keys().map(|s| s.as_str()).collect()
    }
}
