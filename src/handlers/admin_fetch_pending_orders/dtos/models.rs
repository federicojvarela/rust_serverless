use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use model::order::OrderSummary;
use mpc_signature_sm::validations::uuid::validate_not_default_uuid;

#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AdminFetchPendingOrdersRequest {
    #[validate(custom = "validate_not_default_uuid")]
    pub key_id: Uuid,
    pub chain_id: u64,
}

#[derive(Serialize, Debug)]
pub struct AdminFetchPendingOrdersResponse {
    pub orders: Vec<OrderSummary>,
    pub order_ids: Vec<String>,
}
