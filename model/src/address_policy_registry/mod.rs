use ethers::types::Address;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq)]
pub enum AddressPolicyRegistryType {
    Default,
    AddressTo { address: Address },
    AddressFrom { address: Address },
}

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq)]
pub struct AddressPolicyRegistry {
    pub client_id: String,
    pub chain_id: u64,
    pub policy: String,
    pub r#type: AddressPolicyRegistryType,
}

pub struct AddressPolicyRegistryBuilder {
    client_id: String,
    chain_id: u64,
    policy: String,
}

impl AddressPolicyRegistryBuilder {
    pub fn new(client_id: String, chain_id: u64, policy: String) -> Self {
        Self {
            client_id,
            chain_id,
            policy,
        }
    }

    pub fn default(self) -> AddressPolicyRegistry {
        AddressPolicyRegistry {
            client_id: self.client_id,
            chain_id: self.chain_id,
            policy: self.policy,
            r#type: AddressPolicyRegistryType::Default,
        }
    }

    pub fn address_to(self, address: Address) -> AddressPolicyRegistry {
        AddressPolicyRegistry {
            client_id: self.client_id,
            chain_id: self.chain_id,
            policy: self.policy,
            r#type: AddressPolicyRegistryType::AddressTo { address },
        }
    }

    pub fn address_from(self, address: Address) -> AddressPolicyRegistry {
        AddressPolicyRegistry {
            client_id: self.client_id,
            chain_id: self.chain_id,
            policy: self.policy,
            r#type: AddressPolicyRegistryType::AddressFrom { address },
        }
    }
}
