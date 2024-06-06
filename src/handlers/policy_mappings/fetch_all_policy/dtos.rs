use model::address_policy_registry::AddressPolicyRegistryType;
use serde::{self, Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Address {
    pub address: String,
    pub policy: String,
    pub r#type: MappingType,
}

#[derive(Deserialize, Serialize)]
pub struct Chain {
    pub chain_id: u64,
    pub addresses: Vec<Address>,
}

#[derive(Deserialize, Serialize)]
pub struct FetchAllPolicyResponse {
    pub chains: Vec<Chain>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingType {
    Default,
    AddressFrom,
    AddressTo,
}

impl From<AddressPolicyRegistryType> for MappingType {
    fn from(value: AddressPolicyRegistryType) -> Self {
        match value {
            AddressPolicyRegistryType::Default => MappingType::Default,
            AddressPolicyRegistryType::AddressFrom { .. } => MappingType::AddressFrom,
            AddressPolicyRegistryType::AddressTo { .. } => MappingType::AddressTo,
        }
    }
}
