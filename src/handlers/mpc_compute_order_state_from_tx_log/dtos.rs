use ethers::types::Log;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct TransactionLogEvent {
    pub detail: Log,
}

#[derive(Serialize, Debug)]
pub struct OrderStateFromTxLog {
    pub order_state: String,
    pub event_name: Option<String>,
    pub event_signature: Option<String>,
}
