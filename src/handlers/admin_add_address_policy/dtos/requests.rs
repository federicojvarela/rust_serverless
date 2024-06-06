use common::deserializers::h160::h160_option;
use ethers::types::H160;
use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryBuilder};
use serde::Deserialize;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AdminAddAddressPolicyRequest {
    #[serde(default, deserialize_with = "h160_option")]
    pub address: Option<H160>,
    pub chain_id: u64,
    pub client_id: String,
    pub policy: String,
    #[serde(default = "default_force")]
    pub force: bool,
}

fn default_force() -> bool {
    false
}

impl From<AdminAddAddressPolicyRequest> for AddressPolicyRegistry {
    fn from(value: AdminAddAddressPolicyRequest) -> Self {
        let builder =
            AddressPolicyRegistryBuilder::new(value.client_id, value.chain_id, value.policy);

        if let Some(address) = value.address {
            builder.address_to(address)
        } else {
            builder.default()
        }
    }
}
