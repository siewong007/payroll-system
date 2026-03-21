use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanySetting {
    pub id: Uuid,
    pub company_id: Uuid,
    pub category: String,
    pub key: String,
    pub value: serde_json::Value,
    pub label: Option<String>,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingRequest {
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateSettingsRequest {
    pub settings: Vec<SettingUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct SettingUpdate {
    pub category: String,
    pub key: String,
    pub value: serde_json::Value,
}
