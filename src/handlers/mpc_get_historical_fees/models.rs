use common::deserializers::u256::decimal_u256;
use ethers::types::U256;
use mpc_signature_sm::model::gas_response::Fees;
use serde::{self, Serialize};

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HistoricalFees {
    /// 0th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub min: U256,

    /// 100th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub max: U256,

    /// 50th percentile
    #[serde(serialize_with = "decimal_u256")]
    pub median: U256,
}

impl Fees for HistoricalFees {}
