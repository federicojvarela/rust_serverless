use ethers::types::{H160, U256};
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub address: H160,
    pub chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub old_nonce: U256,
    pub new_nonce: U256,
}
