use ethers::prelude::transaction::eip712::TypedData;
use ethers::types::{Bytes, H160, H256, U256};
use mpc_signature_sm::dtos::requests::transaction_request::TransactionRequest;
use serde::Serialize;
use uuid::Uuid;

use crate::models::MaestroSignResponse;

#[derive(Serialize, Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct SignatureResponse {
    pub transaction: TransactionResponse,
    pub key_id: Uuid,
    #[serde(flatten)]
    pub approval: MaestroApproval,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "approval_status")]
#[serde(rename_all = "lowercase")]
pub enum MaestroApproval {
    Approved {
        #[serde(rename(serialize = "maestro_signature"))]
        signature: String,
        transaction_hash: H256,
    },
    Rejected {
        reason: String,
    },
}

impl From<MaestroSignResponse> for MaestroApproval {
    fn from(response: MaestroSignResponse) -> Self {
        match response {
            MaestroSignResponse::Approved {
                signature,
                transaction_hash,
            } => MaestroApproval::Approved {
                signature,
                transaction_hash,
            },
            MaestroSignResponse::Rejected { reason } => MaestroApproval::Rejected { reason },
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(Clone))]
#[serde(untagged)]
pub enum TransactionResponse {
    Legacy {
        #[serde(deserialize_with = "h160")]
        to: H160,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas_price: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        value: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        nonce: U256,

        #[serde(deserialize_with = "bytes")]
        data: Bytes,

        chain_id: u64,
    },
    Eip1559 {
        #[serde(deserialize_with = "h160")]
        to: H160,

        #[serde(deserialize_with = "unsigned_integer_256")]
        gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        max_fee_per_gas: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        value: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        nonce: U256,

        #[serde(deserialize_with = "unsigned_integer_256")]
        max_priority_fee_per_gas: U256,

        #[serde(deserialize_with = "bytes")]
        data: Bytes,

        chain_id: u64,
    },
    // EIP-712
    Sponsored {
        typed_data: TypedData,
        chain_id: u64,
    },
}

impl From<TransactionRequest> for TransactionResponse {
    fn from(transaction: TransactionRequest) -> Self {
        match transaction {
            TransactionRequest::Legacy {
                to,
                gas,
                gas_price,
                value,
                nonce,
                data,
                chain_id,
            } => Self::Legacy {
                to,
                gas,
                gas_price,
                value,
                nonce,
                data,
                chain_id,
            },
            TransactionRequest::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                value,
                nonce,
                max_priority_fee_per_gas,
                data,
                chain_id,
            } => Self::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                value,
                nonce,
                data,
                chain_id,
            },
            TransactionRequest::Sponsored {
                chain_id,
                typed_data,
            } => Self::Sponsored {
                chain_id,
                typed_data,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    impl MaestroApproval {
        pub fn as_result(&self) -> Result<(&String, &H256), &String> {
            match self {
                MaestroApproval::Approved {
                    signature,
                    transaction_hash,
                } => Ok((signature, transaction_hash)),
                MaestroApproval::Rejected { reason } => Err(reason),
            }
        }
    }
}
