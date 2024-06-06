use anyhow::{anyhow, Error};
use common::deserializers::{
    bytes::bytes, string_or_h160::from_string_or_h160, u256::unsigned_integer_256,
};
use ethers::prelude::transaction::eip712::TypedData;
use ethers::types::{Address, Bytes, H160, U256};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum OrderTransaction {
    Legacy {
        #[serde(deserialize_with = "from_string_or_h160")]
        to: String,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas_price: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        value: U256,

        #[serde(deserialize_with = "bytes")]
        data: Bytes,

        chain_id: u64,

        #[serde(skip_serializing_if = "Option::is_none")]
        nonce: Option<U256>,
    },
    Eip1559 {
        #[serde(deserialize_with = "from_string_or_h160")]
        to: String,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        max_fee_per_gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        value: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        max_priority_fee_per_gas: U256,

        #[serde(deserialize_with = "bytes")]
        data: Bytes,

        chain_id: u64,

        #[serde(skip_serializing_if = "Option::is_none")]
        nonce: Option<U256>,
    },
    // EIP-712
    Sponsored {
        to: Address,

        typed_data: TypedData,

        chain_id: u64,

        sponsor_addresses: SponsorAddresses,
    },
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct SponsorAddresses {
    pub gas_pool_address: Address,
    pub forwarder_address: Address,
    pub forwarder_name: String,
}

#[derive(Deserialize, Debug, Default, Serialize, Clone, PartialEq)]
pub struct OrderData<T> {
    #[serde(flatten)]
    pub shared_data: SharedOrderData,

    #[serde(flatten)]
    pub data: T,
}

#[derive(Deserialize, Debug, Default, Serialize, Clone, PartialEq)]
pub struct SharedOrderData {
    pub client_id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SignatureOrderData {
    pub transaction: OrderTransaction,
    pub address: H160,
    pub key_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maestro_signature: Option<Bytes>,
}

pub type GenericOrderData = OrderData<serde_json::Value>;

impl GenericOrderData {
    pub fn extract_address_and_chain_id(&self) -> Result<(H160, u64), Error> {
        let address = self
            .extract_and_convert_address()
            .ok_or(anyhow!("Failed to find address for the order"))?;
        let chain_id = self
            .extract_and_convert_chain_id()
            .ok_or(anyhow!("Failed to find chain_id for the order"))?;
        Ok((address, chain_id))
    }

    fn extract_and_convert_address(&self) -> Option<H160> {
        if let Some(address_value) = self.data.get("address") {
            if let Some(address_str) = address_value.as_str() {
                if let Ok(address) = H160::from_str(address_str) {
                    return Some(address);
                }
            }
        }
        None
    }

    pub fn extract_and_convert_chain_id(&self) -> Option<u64> {
        if let Some(transaction) = self.data.get("transaction") {
            if let Some(chain_id_value) = transaction.get("chain_id") {
                if let Some(chain_id) = chain_id_value.as_u64() {
                    return Some(chain_id);
                }
            }
        }
        None
    }
}
