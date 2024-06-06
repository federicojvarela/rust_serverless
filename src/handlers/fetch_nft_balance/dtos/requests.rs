use common::deserializers::h160::from_array_h160;
use ethers::types::Address;
use mpc_signature_sm::blockchain::providers::{Pagination, PAGE_SIZE_DEFAULT};
use serde::Deserialize;
use validator::Validate;

#[cfg(test)]
use serde::Serialize;

#[derive(Validate, Deserialize, Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct NftBalanceRequest {
    #[serde(deserialize_with = "from_array_h160")]
    #[validate(length(min = 1, max = 45))]
    pub contract_addresses: Vec<Address>,
    #[validate]
    pub pagination: Option<PaginationRequest>,
}

#[derive(Validate, Deserialize, Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct PaginationRequest {
    #[validate(range(min = 1, max = 100))]
    pub page_size: Option<u32>,
    pub page_key: Option<String>,
}

impl From<PaginationRequest> for Pagination {
    fn from(value: PaginationRequest) -> Self {
        Self {
            page_size: value.page_size.unwrap_or(PAGE_SIZE_DEFAULT),
            page_key: value.page_key,
        }
    }
}
