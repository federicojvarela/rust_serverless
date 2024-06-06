use chrono::{DateTime, Utc};
use common::deserializers::u64::{maybe_from_str_u64, str_u64};
use model::order::OrderState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct TransactionIncludedInBlockEvent {
    pub detail: Detail,
}

#[derive(Serialize, Debug)]
pub struct MpcUpdateOrderResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<Uuid>,
}

#[derive(Deserialize, Debug)]
pub struct Detail {
    pub hash: String,
    pub from: String,
    #[serde(rename = "chainId")]
    #[serde(deserialize_with = "maybe_from_str_u64")]
    pub chain_id: u64,
    #[serde(rename = "blockNumber")]
    #[serde(deserialize_with = "str_u64")]
    pub block_number: u64,
    #[serde(rename = "blockHash")]
    pub block_hash: String,
}

#[derive(Serialize)]
pub struct TransactionState {
    #[serde(rename(serialize = ":state"))]
    pub state: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Order {
    pub order_id: Uuid,
    pub state: OrderState,
    pub replaced_by: Option<Uuid>,
    pub replaces: Option<Uuid>,
}

#[derive(Serialize)]
pub struct UpdateState {
    #[serde(rename(serialize = ":state"))]
    pub state: OrderState,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
    #[serde(rename(serialize = ":block_number"))]
    pub block_number: u64,
    #[serde(rename(serialize = ":block_hash"))]
    pub block_hash: String,
}
