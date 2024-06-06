use anyhow::anyhow;
use common::deserializers::{bytes::bytes, h160::h160, u256::unsigned_integer_256};
use ethers::prelude::transaction::eip712::TypedData;
use ethers::types::transaction::eip712::Eip712;
use ethers::types::{Bytes, H160, U256};
use rlp::RlpStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
}

// Used by:
// - mpc_default_approver
// - maestro_sign_request
/// This DTO contains all the supported transactions. Thanks to the `#[serde(untagged)]` macro this
/// will be transparent for the user.
/// This design also prepares the code for supporting other types of transactions (Evm based or
/// not) in the future.
#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum TransactionRequest {
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

impl TransactionRequest {
    // NOTE: This function is specific for EVM transactions. We have to rethink this design when we
    // add different kind of transactions.
    pub fn set_nonce<T: Into<U256>>(&mut self, new_nonce: T) {
        match self {
            TransactionRequest::Legacy { ref mut nonce, .. }
            | TransactionRequest::Eip1559 { ref mut nonce, .. } => {
                *nonce = new_nonce.into();
            }
            TransactionRequest::Sponsored { .. } => {}
        }
    }

    // NOTE: This function is specific for EVM transactions. We have to rethink this design when we
    // add different kind of transactions.
    pub fn get_chain_id(&self) -> u64 {
        match self {
            TransactionRequest::Legacy { chain_id, .. }
            | TransactionRequest::Eip1559 { chain_id, .. }
            | TransactionRequest::Sponsored { chain_id, .. } => *chain_id,
        }
    }

    // NOTE: This should be in a business model struct and not in a DTO, but since we are using it
    // as both is here.
    // NOTE: This design is not correct if we are going to support transaction that does not
    // support RLP encoding (evm or from other chains) so we will have to rethink this when it come
    // the time to support other transactions.
    /// This method encodes the transaction into the RLP form.
    ///
    /// Based on implementation here:
    /// https://github.com/gakonst/ethers-rs/blob/master/ethers-core/src/types/transaction/request.rs#L162
    /// Transaction types taken from:
    /// https://docs.infura.io/networks/ethereum/concepts/transaction-types
    pub fn as_rlp(&self) -> Result<Vec<u8>, TransactionError> {
        let mut rlp = RlpStream::new();
        rlp.begin_list(9);

        match self {
            TransactionRequest::Legacy {
                to,
                gas,
                gas_price,
                value,
                nonce,
                data,
                chain_id,
            } => {
                rlp.append(nonce);
                rlp.append(gas_price);
                rlp.append(gas);

                // To deploy smart contracts
                if to == &H160::zero() {
                    rlp.append(&0x0u8);
                } else {
                    rlp.append(to);
                }

                rlp.append(value);

                rlp.append(&data.0);

                rlp.append(chain_id);
                rlp.append(&0u8);
                rlp.append(&0u8);

                Ok(rlp.out().freeze().into())
            }
            TransactionRequest::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                value,
                nonce,
                max_priority_fee_per_gas,
                data,
                chain_id,
            } => {
                // append chain_id. from EIP-2930: chainId is defined to be an integer of arbitrary size.
                rlp.append(chain_id);

                rlp.append(nonce);
                rlp.append(max_priority_fee_per_gas);
                rlp.append(max_fee_per_gas);
                rlp.append(gas);

                // To deploy smart contracts
                if to == &H160::zero() {
                    rlp.append(&0x0u8);
                } else {
                    rlp.append(to);
                }

                rlp.append(value);
                rlp.append(&data.0);
                rlp.begin_list(0);

                Ok([&[2u8], rlp.as_raw()].concat())
            }
            TransactionRequest::Sponsored {
                typed_data,
                chain_id: _,
            } => {
                let encode = typed_data
                    .encode_eip712()
                    .map_err(|e| TransactionError::Unknown(anyhow!(e)))?;

                Ok(encode.to_vec())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    const TX_RLP_LEGACY: &str = "0xf83980820100833000009425dfe735c17fec1d86a458657189060d65be69a80194640651604161065132510616516510651616961083aa36a78080";
    const TX_RLP_EIP1559: &str = "0x02f83c83aa36a78083080000820100833000009425dfe735c17fec1d86a458657189060d65be69a801946406516041610651325106165165106516169610c0";
    const TX_RLP_0X0_LEGACY: &str =
        "0xe58082010083300000800194640651604161065132510616516510651616961083aa36a78080";
    const TX_RLP_0X0_EIP1559: &str =
        "0x02e883aa36a78083080000820100833000008001946406516041610651325106165165106516169610c0";

    #[test]
    pub fn rlp_legacy_is_correct() {
        let transaction = TransactionRequest::Legacy {
            to: H160::from_str("0x25DFE735C17FEC1d86A458657189060D65Be69a8").unwrap(),
            gas: "300000".into(),
            gas_price: "100".into(),
            value: "1".into(),
            nonce: 0.into(),
            data: Bytes::from_str("0x6406516041610651325106165165106516169610").unwrap(),
            chain_id: 11155111,
        };

        let rlp = transaction.as_rlp().unwrap();
        assert_eq!(Bytes::from_str(TX_RLP_LEGACY).unwrap(), Bytes::from(rlp));
    }

    #[test]
    pub fn rlp_legacy_to_0x0_is_correct() {
        let transaction = TransactionRequest::Legacy {
            to: H160::zero(),
            gas: "300000".into(),
            gas_price: "100".into(),
            value: "1".into(),
            nonce: 0.into(),
            data: Bytes::from_str("0x6406516041610651325106165165106516169610").unwrap(),
            chain_id: 11155111,
        };

        let rlp = transaction.as_rlp().unwrap();

        assert_eq!(
            Bytes::from_str(TX_RLP_0X0_LEGACY).unwrap(),
            Bytes::from(rlp)
        );
    }

    #[test]
    pub fn rlp_eip1559_is_correct() {
        let transaction = TransactionRequest::Eip1559 {
            to: H160::from_str("0x25DFE735C17FEC1d86A458657189060D65Be69a8").unwrap(),
            gas: "300000".into(),
            max_fee_per_gas: "100".into(),
            max_priority_fee_per_gas: "80000".into(),
            value: "1".into(),
            nonce: 0.into(),
            data: Bytes::from_str("0x6406516041610651325106165165106516169610").unwrap(),
            chain_id: 11155111,
        };

        let rlp = transaction.as_rlp().unwrap();
        assert_eq!(Bytes::from_str(TX_RLP_EIP1559).unwrap(), Bytes::from(rlp));
    }

    #[test]
    pub fn rlp_eip1559_to_0x0_is_correct() {
        let transaction = TransactionRequest::Eip1559 {
            to: H160::zero(),
            gas: "300000".into(),
            max_fee_per_gas: "100".into(),
            max_priority_fee_per_gas: "80000".into(),
            value: "1".into(),
            nonce: 0.into(),
            data: Bytes::from_str("0x6406516041610651325106165165106516169610").unwrap(),
            chain_id: 11155111,
        };

        let rlp = transaction.as_rlp().unwrap();
        assert_eq!(
            Bytes::from_str(TX_RLP_0X0_EIP1559).unwrap(),
            Bytes::from(rlp)
        );
    }
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum TransactionRequestNoNonce {
    Legacy {
        #[serde(deserialize_with = "h160")]
        to: H160,

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
        #[serde(default, deserialize_with = "h160")]
        to: H160,

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

impl TransactionRequestNoNonce {
    pub fn into_transaction_request_with_nonce(self, nonce: U256) -> TransactionRequest {
        match self {
            TransactionRequestNoNonce::Legacy {
                to,
                gas,
                gas_price,
                value,
                data,
                chain_id,
            } => TransactionRequest::Legacy {
                to,
                gas,
                gas_price,
                value,
                nonce,
                data,
                chain_id,
            },
            TransactionRequestNoNonce::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                value,
                max_priority_fee_per_gas,
                data,
                chain_id,
            } => TransactionRequest::Eip1559 {
                to,
                gas,
                max_fee_per_gas,
                value,
                nonce,
                max_priority_fee_per_gas,
                data,
                chain_id,
            },
        }
    }
}
