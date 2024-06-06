use crate::dtos::requests::InvalidTransactionRequestError::{
    ChainIdNotSupported, EmptyToAddress, MaxPriorityFeeBiggerThanMaxFee,
};
use common::deserializers::bytes::bytes;
use common::deserializers::string_or_h160::from_string_or_h160;
use common::deserializers::u256::unsigned_integer_256;
use ethers::types::{Bytes, U256};
use http::Response;
use model::order::OrderTransaction;
use mpc_signature_sm::config::SupportedChain;
use mpc_signature_sm::http::errors::validation_error_response;
use serde::{Deserialize, Serialize};

/*
 * Value types to test in integration tests:
 * Uuid, H160, U256, Bytes (hex string).
 */
#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct SignatureRequestBody {
    pub transaction: TransactionRequest,
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum TransactionRequest {
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
    },
    Eip1559 {
        #[serde(default, deserialize_with = "from_string_or_h160")]
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
    },
}

impl TransactionRequest {
    fn validate_chain_id(&self, chain_id: &u64) -> Result<(), InvalidTransactionRequestError> {
        if !chain_id.is_supported() {
            Err(ChainIdNotSupported(*chain_id))
        } else {
            Ok(())
        }
    }
    fn validate_to_address_not_empty(
        &self,
        to: &String,
    ) -> Result<(), InvalidTransactionRequestError> {
        if to.is_empty() {
            Err(EmptyToAddress)
        } else {
            Ok(())
        }
    }

    fn validate_eip_1559_gas(
        &self,
        max_fee_per_gas: &U256,
        max_priority_fee_per_gas: &U256,
    ) -> Result<(), InvalidTransactionRequestError> {
        if max_priority_fee_per_gas > max_fee_per_gas {
            Err(MaxPriorityFeeBiggerThanMaxFee)
        } else {
            Ok(())
        }
    }

    pub fn validate(&self) -> Result<(), InvalidTransactionRequestError> {
        match self {
            TransactionRequest::Legacy { chain_id, to, .. } => {
                self.validate_chain_id(chain_id)?;
                self.validate_to_address_not_empty(to)?;
            }
            TransactionRequest::Eip1559 {
                chain_id,
                to,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                ..
            } => {
                self.validate_chain_id(chain_id)?;
                self.validate_to_address_not_empty(to)?;
                self.validate_eip_1559_gas(max_fee_per_gas, max_priority_fee_per_gas)?;
            }
        };

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidTransactionRequestError {
    #[error("to address cannot be empty")]
    EmptyToAddress,

    #[error("max_priority_fee_per_gas cannot be bigger than max_fee_per_gas")]
    MaxPriorityFeeBiggerThanMaxFee,

    #[error("chain_id {0} is not supported")]
    ChainIdNotSupported(u64),
}

impl From<InvalidTransactionRequestError> for Response<String> {
    fn from(e: InvalidTransactionRequestError) -> Self {
        validation_error_response(e.to_string(), None)
    }
}

impl From<&TransactionRequest> for OrderTransaction {
    fn from(value: &TransactionRequest) -> Self {
        match value {
            TransactionRequest::Legacy {
                to,
                gas,
                gas_price,
                value,
                data,
                chain_id,
            } => OrderTransaction::Legacy {
                to: to.clone(),
                gas: *gas,
                gas_price: *gas_price,
                value: *value,
                data: data.clone(),
                chain_id: *chain_id,
                nonce: None,
            },
            TransactionRequest::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                value,
                max_priority_fee_per_gas,
                data,
                chain_id,
            } => OrderTransaction::Eip1559 {
                to: to.clone(),
                gas: *gas,
                max_fee_per_gas: *max_fee_per_gas,
                value: *value,
                max_priority_fee_per_gas: *max_priority_fee_per_gas,
                data: data.clone(),
                chain_id: *chain_id,
                nonce: None,
            },
        }
    }
}
