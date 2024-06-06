use common::deserializers::u256::unsigned_integer_256;
use ethers::types::U256;
use serde::Deserialize;

#[cfg(test)]
use serde::Serialize;

#[derive(Deserialize)]
#[cfg_attr(test, derive(Serialize))]
pub struct ReplacementRequest {
    pub transaction: ReplacementRequestType,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(Serialize))]
#[serde(untagged)]
pub enum ReplacementRequestType {
    Legacy {
        #[serde(deserialize_with = "unsigned_integer_256")]
        gas_price: U256,
    },
    Eip1559 {
        #[serde(deserialize_with = "unsigned_integer_256")]
        max_fee_per_gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        max_priority_fee_per_gas: U256,
    },
}
