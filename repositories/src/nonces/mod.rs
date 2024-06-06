use crate::impl_unknown_error_trait;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ethers::types::{Address, H160, H256, U256};
use model::nonce::Nonce;
use serde::Serialize;

#[cfg(feature = "test_mocks")]
use mockall::mock;

pub mod nonces_repository_impl;

#[derive(Debug, thiserror::Error)]
pub enum NoncesRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
    #[error("{0}")]
    NonceNotFound(String),
    #[error("{0}")]
    ConditionalCheckFailed(String),
}

impl_unknown_error_trait!(NoncesRepositoryError);

#[derive(Debug, Serialize)]
pub struct NonceUpdateDynamoDbResource {
    #[serde(rename(serialize = ":new_nonce"))]
    pub new_nonce: U256,

    #[serde(rename(serialize = ":current_nonce"))]
    pub current_nonce: U256,

    #[serde(rename(serialize = ":transaction_hash"))]
    pub transaction_hash: String,

    #[serde(rename(serialize = ":created_at"))]
    pub created_at: DateTime<Utc>,

    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct NonceSetDynamoDbResource {
    #[serde(rename(serialize = ":nonce"))]
    pub nonce: U256,

    #[serde(rename(serialize = ":transaction_hash"))]
    pub transaction_hash: String,

    #[serde(rename(serialize = ":created_at"))]
    pub created_at: DateTime<Utc>,

    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NoncePrimaryKeyDynamoDbResource {
    pub address: H160,
    pub chain_id: u64,
}

#[async_trait]
pub trait NoncesRepository
where
    Self: Sync + Send,
{
    async fn get_nonce(
        &self,
        address: Address,
        chain_id: u64,
    ) -> Result<Nonce, NoncesRepositoryError>;

    async fn increment_nonce(
        &self,
        address: Address,
        current_nonce: U256,
        hash: String,
        chain_id: u64,
    ) -> Result<(), NoncesRepositoryError>;

    async fn set_nonce(
        &self,
        address: Address,
        current_nonce: U256,
        hash: Option<H256>,
        chain_id: u64,
    ) -> Result<(), NoncesRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub NoncesRepository {}
    #[async_trait]
    impl NoncesRepository for NoncesRepository {
        async fn get_nonce(
            &self,
            address: Address,
            chain_id: u64,
        ) -> Result<Nonce, NoncesRepositoryError>;

        async fn increment_nonce(
            &self,
            address: Address,
            current_nonce: U256,
            hash: String,
            chain_id: u64,
        ) -> Result<(), NoncesRepositoryError>;

        async fn set_nonce(
            &self,
            address: Address,
            current_nonce: U256,
            hash: Option<H256>,
            chain_id: u64,
        ) -> Result<(), NoncesRepositoryError>;
    }
}
