use crate::impl_unknown_error_trait;
use async_trait::async_trait;
use ethers::types::Address;
use model::key::Key;
use serde::Serialize;

pub mod keys_repository_impl;

#[cfg(feature = "test_mocks")]
use mockall::mock;

const KEY_ADDRESS_INDEX_NAME: &str = "AddressIndex";

#[derive(Serialize)]
struct KeyAddressGSI {
    #[serde(rename(serialize = ":address"))]
    pub address: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KeysRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
    #[error("{0}")]
    KeyNotFound(String),
}

impl_unknown_error_trait!(KeysRepositoryError);

#[async_trait]
pub trait KeysRepository
where
    Self: Sync + Send,
{
    async fn get_key_by_address(&self, address: Address) -> Result<Key, KeysRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub KeysRepository {}
    #[async_trait]
    impl KeysRepository for KeysRepository {
        async fn get_key_by_address(&self, address: Address) -> Result<Key, KeysRepositoryError>;
    }
}
