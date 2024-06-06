use common::deserializers::h160::h160;
use ethers::types::Address;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct SelectPolicyRequest {
    pub client_id: String,
    pub chain_id: u64,
    #[serde(deserialize_with = "h160")]
    pub address: Address,
}

#[derive(Serialize, Debug)]
pub struct SelectPolicyResponse {
    pub policy_name: String,
}
