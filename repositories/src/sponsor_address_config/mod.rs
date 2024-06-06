pub mod sponsor_address_config_repository_impl;

use crate::impl_unknown_error_trait;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ethers::types::Address;
use model::sponsor_address_config::{SponsorAddressConfig, SponsorAddressConfigType};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[cfg(feature = "test_mocks")]
use mockall::mock;

#[derive(Serialize)]
pub struct SponsorAddressConfigPk {
    #[serde(rename(serialize = ":pk"))]
    pub pk: String,
}

impl SponsorAddressConfigPk {
    pub fn new(client_id: String, chain_id: u64, address_type: SponsorAddressConfigType) -> Self {
        let address_type = address_type.as_str();
        Self {
            pk: format!("CLIENT#{client_id}#CHAIN_ID#{chain_id}#ADDRESS_TYPE#{address_type}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SponsorAddressConfigRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
}

impl_unknown_error_trait!(SponsorAddressConfigRepositoryError);

#[derive(Deserialize, Serialize, Debug)]
pub struct SponsorAddressConfigDynamoDbResource {
    pub pk: String,
    pub sk: String,
    pub client_id: String,
    pub chain_id: u64,
    pub address_type: String,
    pub address: String,
    pub forwarder_name: Option<String>,
    pub last_modified_at: DateTime<Utc>,
}

impl TryFrom<SponsorAddressConfigDynamoDbResource> for SponsorAddressConfig {
    type Error = SponsorAddressConfigRepositoryError;

    fn try_from(value: SponsorAddressConfigDynamoDbResource) -> Result<Self, Self::Error> {
        let address_type: SponsorAddressConfigType =
            SponsorAddressConfigType::from_str(value.address_type.as_str())
                .map_err(SponsorAddressConfigRepositoryError::Unknown)?;

        let address = Address::from_str(&value.address).map_err(|e| {
            SponsorAddressConfigRepositoryError::Unknown(
                anyhow!(e).context("Unable to convert address to address type h160"),
            )
        })?;

        Ok(match address_type {
            SponsorAddressConfigType::GasPool => Self::GasPool {
                client_id: value.client_id,
                chain_id: value.chain_id,
                address,
            },
            SponsorAddressConfigType::Forwarder => {
                let forwarder_name = value.forwarder_name.ok_or("").map_err(|e| {
                    SponsorAddressConfigRepositoryError::Unknown(
                        anyhow!(e)
                            .context("Missing forwarder name in entry of address_type FORWARDER"),
                    )
                })?;
                Self::Forwarder {
                    client_id: value.client_id,
                    chain_id: value.chain_id,
                    address,
                    forwarder_name,
                }
            }
        })
    }
}

#[async_trait]
pub trait SponsorAddressConfigRepository
where
    Self: Sync + Send,
{
    async fn get_addresses(
        &self,
        client_id: String,
        chain_id: u64,
        address_type: SponsorAddressConfigType,
    ) -> Result<Vec<SponsorAddressConfig>, SponsorAddressConfigRepositoryError>;

    async fn put_address_gas_pool(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
    ) -> Result<(), SponsorAddressConfigRepositoryError>;

    async fn put_address_forwarder(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
        forwarder_name: String,
    ) -> Result<(), SponsorAddressConfigRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub SponsorAddressConfigRepository {}
    #[async_trait]
    impl SponsorAddressConfigRepository for SponsorAddressConfigRepository {
       async fn get_addresses(
            &self,
            client_id: String,
            chain_id: u64,
            address_type: SponsorAddressConfigType,
        ) -> Result<Vec<SponsorAddressConfig>, SponsorAddressConfigRepositoryError>;

        async fn put_address_gas_pool(
            &self,
            client_id: String,
            chain_id: u64,
            address: Address,
        ) -> Result<(), SponsorAddressConfigRepositoryError>;

        async fn put_address_forwarder(
            &self,
            client_id: String,
            chain_id: u64,
            address: Address,
            forwarder_name: String,
        ) -> Result<(), SponsorAddressConfigRepositoryError>;
    }
}
