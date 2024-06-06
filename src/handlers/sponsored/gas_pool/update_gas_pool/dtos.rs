use common::deserializers::h160::h160;
use ethers::types::Address;
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct UpdateGasPoolRequest {
    #[serde(deserialize_with = "h160")]
    pub gas_pool_address: Address,
}
