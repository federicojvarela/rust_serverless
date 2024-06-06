use common::deserializers::h160::h160;
use ethers::types::H160;
use mpc_signature_sm::validations::http::supported_chain_id::is_supported_chain_id;
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreatePolicyMappingRequest {
    #[serde(default, deserialize_with = "h160")]
    pub address: H160,

    #[validate(custom = "is_supported_chain_id")]
    pub chain_id: u64,

    pub policy: String,
}
