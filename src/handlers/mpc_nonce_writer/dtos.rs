use common::deserializers::{h160::h160, u64::maybe_from_str_u64};
use ethers::types::{H160, U256};
use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct ChainListenerEvent {
    pub detail: Transaction,
}

#[derive(Debug, Deserialize, Validate)]
pub struct Transaction {
    #[serde(deserialize_with = "h160")]
    pub from: H160,

    pub hash: String,

    pub nonce: U256,

    #[serde(
        deserialize_with = "maybe_from_str_u64",
        rename(deserialize = "chainId")
    )]
    pub chain_id: u64,
}
