pub mod address_policy_registry_repository_impl;

use std::str::FromStr;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::serializers::h160::h160_to_lowercase_hex_string;
use ethers::types::Address;
use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryType};
use serde::{Deserialize, Serialize};

const CLIENT_ID_INDEX_NAME: &str = "client_id_index";
const TYPE_ADDRESS_FROM: &str = "ADDRESS_FROM";
const TYPE_ADDRESS_TO: &str = "ADDRESS";

#[cfg(feature = "test_mocks")]
use mockall::mock;

use crate::{deserialize::UnknownError, impl_unknown_error_trait};

#[derive(Serialize, Clone)]
pub struct AddressPolicyRegistryPk {
    pub pk: String,
    pub sk: String,
}

#[derive(Serialize)]
pub struct ClientIdGSI {
    #[serde(rename(serialize = ":client_id"))]
    pub client_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AddressPolicyRegistryRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
    #[error("{0}")]
    PolicyNotFound(String),
}

impl From<anyhow::Error> for AddressPolicyRegistryRepositoryError {
    fn from(error: anyhow::Error) -> Self {
        AddressPolicyRegistryRepositoryError::Unknown(error)
    }
}

impl ClientIdGSI {
    pub fn new(client: String) -> Self {
        Self {
            client_id: format!("CLIENT#{client}"),
        }
    }
}

impl_unknown_error_trait!(AddressPolicyRegistryRepositoryError);

impl AddressPolicyRegistryPk {
    pub fn new(client: String, chain_id: u64, address_to: Option<Address>) -> Self {
        Self {
            pk: format!("CLIENT#{client}#CHAIN_ID#{chain_id}"),
            sk: format!(
                "{TYPE_ADDRESS_TO}#{}",
                address_to
                    .map(h160_to_lowercase_hex_string)
                    .unwrap_or("DEFAULT".to_owned())
            ),
        }
    }
    pub fn new_from_address(client: String, chain_id: u64, address_from: Address) -> Self {
        Self {
            pk: format!("CLIENT#{client}#CHAIN_ID#{chain_id}"),
            sk: format!(
                "{TYPE_ADDRESS_FROM}#{}",
                h160_to_lowercase_hex_string(address_from)
            ),
        }
    }
}

#[derive(Serialize)]
struct UpdatePolicy {
    #[serde(rename(serialize = ":policy"))]
    pub policy: String,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Deserialize, Serialize)]
pub struct AddressPolicyRegistryDynamoDbResource {
    pub pk: String,
    pub sk: String,
    pub client_id: String,
    pub chain_id: u64,
    pub policy: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

fn parse_address_from_option(
    address: Option<String>,
) -> Result<Address, AddressPolicyRegistryRepositoryError> {
    let address = address.ok_or_else(|| {
        AddressPolicyRegistryRepositoryError::Unknown(anyhow!("address not found in registry"))
    })?;

    Address::from_str(&address).map_err(|e| {
        AddressPolicyRegistryRepositoryError::unknown(e, Some("unable to parse address"))
    })
}

impl TryFrom<AddressPolicyRegistryDynamoDbResource> for AddressPolicyRegistry {
    type Error = AddressPolicyRegistryRepositoryError;

    fn try_from(value: AddressPolicyRegistryDynamoDbResource) -> Result<Self, Self::Error> {
        let mapping_type: Vec<&str> = value.sk.split('#').collect();
        let mapping_type = match mapping_type.first() {
            Some(&TYPE_ADDRESS_TO) => match mapping_type.get(1) {
                Some(&"DEFAULT") => AddressPolicyRegistryType::Default,
                Some(_) => AddressPolicyRegistryType::AddressTo {
                    address: parse_address_from_option(value.address)?,
                },
                None => {
                    return Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                        "malformed sort key for pk {}, address or default not found",
                        value.pk
                    )))
                }
            },
            Some(&TYPE_ADDRESS_FROM) => AddressPolicyRegistryType::AddressTo {
                address: parse_address_from_option(value.address)?,
            },
            Some(_) => {
                return Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                    "invalid address mapping type found for pk {}",
                    value.pk
                )))
            }
            None => {
                return Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                    "no sort key found for pk {}",
                    value.pk
                )))
            }
        };

        Ok(Self {
            client_id: value.client_id,
            chain_id: value.chain_id,
            policy: value.policy,
            r#type: mapping_type,
        })
    }
}

impl From<AddressPolicyRegistry> for AddressPolicyRegistryDynamoDbResource {
    fn from(value: AddressPolicyRegistry) -> Self {
        match value.r#type {
            AddressPolicyRegistryType::Default => {
                let key =
                    AddressPolicyRegistryPk::new(value.client_id.clone(), value.chain_id, None);
                Self {
                    pk: key.pk,
                    sk: key.sk,
                    client_id: value.client_id,
                    chain_id: value.chain_id,
                    policy: value.policy,
                    created_at: Utc::now(),
                    address: None,
                }
            }
            AddressPolicyRegistryType::AddressTo { address } => {
                let key = AddressPolicyRegistryPk::new(
                    value.client_id.clone(),
                    value.chain_id,
                    Some(address),
                );
                Self {
                    pk: key.pk,
                    sk: key.sk,
                    client_id: value.client_id,
                    chain_id: value.chain_id,
                    policy: value.policy,
                    created_at: Utc::now(),
                    address: Some(h160_to_lowercase_hex_string(address)),
                }
            }
            AddressPolicyRegistryType::AddressFrom { address } => {
                let key = AddressPolicyRegistryPk::new_from_address(
                    value.client_id.clone(),
                    value.chain_id,
                    address,
                );

                Self {
                    pk: key.pk,
                    sk: key.sk,
                    client_id: value.client_id,
                    chain_id: value.chain_id,
                    policy: value.policy,
                    created_at: Utc::now(),
                    address: Some(h160_to_lowercase_hex_string(address)),
                }
            }
        }
    }
}

#[async_trait]
pub trait AddressPolicyRegistryRepository
where
    Self: Sync + Send,
{
    async fn get_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<Address>,
    ) -> Result<Option<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError>;

    async fn put_policy(
        &self,
        policy_mapping: AddressPolicyRegistry,
    ) -> Result<(), AddressPolicyRegistryRepositoryError>;

    async fn delete_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<Address>,
    ) -> Result<(), AddressPolicyRegistryRepositoryError>;

    async fn get_all_policies(
        &self,
        client_id: String,
    ) -> Result<Vec<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError>;

    async fn update_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<Address>,
        policy: String,
    ) -> Result<(), AddressPolicyRegistryRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub AddressPolicyRegistryRepository {}
    #[async_trait]
    impl AddressPolicyRegistryRepository for AddressPolicyRegistryRepository {
        async fn get_policy(
                &self,
                client_id: String,
                chain_id: u64,
                address: Option<Address>,
            ) -> Result<Option<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError>;

        async fn put_policy(
            &self,
            policy_mapping: AddressPolicyRegistry,
        ) -> Result<(), AddressPolicyRegistryRepositoryError>;

        async fn get_all_policies(
            &self,
            client_id: String,
        ) -> Result<Vec<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError>;

        async fn delete_policy(
            &self,
            client_id: String,
            chain_id: u64,
            address: Option<Address>,
        ) -> Result<(), AddressPolicyRegistryRepositoryError>;

        async fn update_policy(
            &self,
            client_id: String,
            chain_id: u64,
            address: Option<Address>,
            policy: String,
        ) -> Result<(), AddressPolicyRegistryRepositoryError>;
    }
}
