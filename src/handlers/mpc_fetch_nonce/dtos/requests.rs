use chrono::{DateTime, Utc};
use ethers::types::{H160, U256};
use serde::{Deserialize, Serialize};
use validator::Validate;

use common::deserializers::h160::h160;
use model::nonce::Nonce;

#[derive(Serialize, Debug)]
pub struct MpcNonceResponse {
    pub address: H160,
    pub chain_id: u64,
    pub nonce: U256,
    pub created_at: DateTime<Utc>,
    pub last_modified_at: DateTime<Utc>,
}

impl MpcNonceResponse {
    pub fn zero_nonce(address: H160, chain_id: u64) -> Self {
        let now = Utc::now();
        MpcNonceResponse {
            address,
            chain_id,
            nonce: 0.into(),
            created_at: now,
            last_modified_at: now,
        }
    }
}
#[derive(Deserialize, Debug, Serialize, Validate, PartialEq, Eq, Clone)]
pub struct MpcNonceRequest {
    #[serde(deserialize_with = "h160")]
    pub address: H160,
    pub chain_id: u64,
}

impl From<Nonce> for MpcNonceResponse {
    fn from(value: Nonce) -> Self {
        Self {
            address: value.address,
            chain_id: value.chain_id,
            nonce: value.nonce,
            created_at: value.created_at,
            last_modified_at: value.last_modified_at,
        }
    }
}
