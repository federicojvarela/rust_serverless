use chrono::{DateTime, Utc};
use model::order::OrderState;
use serde::{Deserialize, Serialize};

// TODO: WALL-1178 -[SPIKE] Rules with no payload require empty json
// This TransactionMonitorRequestEvent is used because scheduled rule requires {} payload
#[derive(Deserialize, Debug)]
pub struct TransactionMonitorRequestEvent {}

#[derive(Serialize)]
pub struct TransactionState {
    #[serde(rename(serialize = ":state"))]
    pub state: String,
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
