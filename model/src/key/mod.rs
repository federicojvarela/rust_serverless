use chrono::{DateTime, Utc};
use serde::{self, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct Key {
    pub key_id: Uuid,
    pub address: String,
    pub client_id: String,
    pub client_user_id: String,
    pub created_at: DateTime<Utc>,
    pub order_type: String,
    pub order_version: String,
    pub owning_user_id: Uuid,
    pub public_key: String,
}
