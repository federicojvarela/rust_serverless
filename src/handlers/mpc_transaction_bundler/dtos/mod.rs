use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub struct TransactionBundlerResponse {
    pub order_id: Uuid,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TransactionBundlerRequest {
    pub maestro_signature: String,
}
