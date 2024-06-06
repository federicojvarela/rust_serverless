use common::deserializers::u256::decimal_u256;
use ethers::types::U256;
use serde::{self, Serialize};

use crate::model::gas_response::Fees;

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct SuggestedFees {
    /// 0th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub low: U256,

    /// 50th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub medium: U256,

    /// 95th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub high: U256,
}

impl Fees for SuggestedFees {}

#[derive(Debug, Serialize, Clone)]
pub struct ProcessedFees {
    pub max_priority_fee_per_gas: SuggestedFees,
    pub max_fee_per_gas: SuggestedFees,
    pub gas_price: SuggestedFees,
}

#[derive(Debug, thiserror::Error)]
pub enum SuggestedFeesProcessingError {
    #[error("the max_priority_fee_per_gas array was empty, check the RPC response")]
    ArrayIsEmpty,
}
