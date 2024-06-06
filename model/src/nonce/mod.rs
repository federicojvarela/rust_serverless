use chrono::{DateTime, Utc};
use common::deserializers::h160::h160;
use ethers::types::{H160, U256};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Nonce {
    #[serde(deserialize_with = "h160")]
    pub address: H160,
    pub chain_id: u64,
    pub nonce: U256,
    pub created_at: DateTime<Utc>,
    pub last_modified_at: DateTime<Utc>,
}
