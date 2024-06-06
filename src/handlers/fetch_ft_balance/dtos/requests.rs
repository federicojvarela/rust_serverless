use common::deserializers::h160::from_array_h160;
use ethers::types::Address;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Debug, Serialize, Validate, Clone)]
pub struct FTBalanceRequest {
    #[serde(deserialize_with = "from_array_h160")]
    #[validate(length(min = 1, max = 100))]
    pub contract_addresses: Vec<Address>,
}
