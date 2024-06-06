use common::deserializers::bytes::bytes;
use common::deserializers::h160::h160;
use common::deserializers::u256::unsigned_integer_256;
use ethers::types::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/*
 * Value types to test in integration tests:
 * Uuid, Address, U256, Bytes (hex string).
 */
#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct SignatureRequestBody {
    pub transaction: TransactionRequest,
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct TransactionRequest {
    #[serde(deserialize_with = "h160")]
    pub to: Address,

    #[serde(deserialize_with = "unsigned_integer_256")]
    pub value: U256,

    #[serde(deserialize_with = "bytes")]
    pub data: Bytes,

    pub deadline: String,

    pub chain_id: u64,
}
