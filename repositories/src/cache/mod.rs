use crate::impl_unknown_error_trait;
use async_trait::async_trait;
use model::cache::{DataType, GenericJsonCache};
use serde::{Deserialize, Serialize};

pub mod cache_repository_impl;

#[cfg(feature = "test_mocks")]
use mockall::mock;

#[derive(Debug, thiserror::Error)]
pub enum CacheRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
    #[error("{0}")]
    KeyNotFound(String),
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct CachePK {
    pub sk: String,
    pub pk: DataType,
}

impl_unknown_error_trait!(CacheRepositoryError);

impl From<anyhow::Error> for CacheRepositoryError {
    fn from(error: anyhow::Error) -> Self {
        CacheRepositoryError::Unknown(error)
    }
}

#[async_trait]
pub trait CacheRepository {
    async fn get_item(
        &self,
        key: &str,
        data_type: DataType,
    ) -> Result<GenericJsonCache, CacheRepositoryError>;
    async fn set_item(&self, value: GenericJsonCache) -> Result<(), CacheRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub CacheRepositoryTest {}

    #[async_trait]
    impl CacheRepository for CacheRepositoryTest {
        async fn get_item(&self, key: &str, data_type: DataType) -> Result<GenericJsonCache, CacheRepositoryError> ;
        async fn set_item(&self, value: GenericJsonCache) -> Result<(), CacheRepositoryError>;
    }
}
