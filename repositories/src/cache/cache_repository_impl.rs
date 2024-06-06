use std::collections::HashMap;

use crate::deserialize::deserialize_from_dynamo;
use anyhow::anyhow;
use async_trait::async_trait;
use model::cache::{DataType, GenericJsonCache};
use rusoto_dynamodb::{AttributeValue, DynamoDb, GetItemInput, PutItemInput};

use super::{CachePK, CacheRepository, CacheRepositoryError};

pub struct CacheRepositoryImpl<T: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: T,
}

impl<T: DynamoDb + Sync + Send> CacheRepositoryImpl<T> {
    pub fn new(table_name: String, dynamodb_client: T) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn build_key_item_input(
        &self,
        entity_key: String,
        data_type: DataType,
    ) -> Result<GetItemInput, anyhow::Error> {
        let key = serde_dynamo::to_item(CachePK {
            sk: entity_key,
            pk: data_type,
        })
        .map_err(|e| anyhow!(e).context("Error building query for cache by key"))?;

        Ok(GetItemInput {
            key,
            table_name: self.table_name.clone(),
            ..GetItemInput::default()
        })
    }

    fn build_create_cache_item_input(
        &self,
        value: GenericJsonCache,
    ) -> Result<PutItemInput, anyhow::Error> {
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(value).map_err(|e| {
            CacheRepositoryError::Unknown(anyhow!(e).context("Error serializing cache item."))
        })?;

        let input = PutItemInput {
            item,
            table_name: self.table_name.clone(),
            ..PutItemInput::default()
        };

        Ok(input)
    }
}

#[async_trait]
impl<T: DynamoDb + Sync + Send> CacheRepository for CacheRepositoryImpl<T> {
    async fn get_item(
        &self,
        key: &str,
        data_type: DataType,
    ) -> Result<GenericJsonCache, CacheRepositoryError> {
        let input = self
            .build_key_item_input(key.to_string(), data_type)
            .map_err(|e| {
                CacheRepositoryError::Unknown(anyhow!(e).context("Error building query for key"))
            })?;

        let result = self
            .dynamodb_client
            .get_item(input)
            .await
            .map_err(|e| {
                CacheRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error querying Cache by key: {key:?}")),
                )
            })?
            .item
            .ok_or_else(|| {
                CacheRepositoryError::KeyNotFound(format!("Cache with key {key:?} not found"))
            })?;

        deserialize_from_dynamo(result)
    }

    async fn set_item(&self, value: GenericJsonCache) -> Result<(), CacheRepositoryError> {
        let new_cache = self.build_create_cache_item_input(value).unwrap();

        self.dynamodb_client
            .put_item(new_cache)
            .await
            .map_err(|e| {
                CacheRepositoryError::Unknown(anyhow!(e).context("Error storing the item in cache"))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::cache::{
        cache_repository_impl::CacheRepositoryImpl, CacheRepository, CacheRepositoryError,
    };
    use chrono::Utc;
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use model::cache::{DataType, GenericJsonCache};
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{GetItemError, GetItemOutput};
    use serde_json::json;

    struct TestFixture {
        pub dynamodb_client: MockDbClient,
        pub table_name: String,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            dynamodb_client: MockDbClient::new(),
            table_name: "cache".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_cache_item_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(GetItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = CacheRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo
            .get_item("key", DataType::FtMetadata)
            .await
            .unwrap_err();
        assert!(matches!(error, CacheRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_cache_item_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| Ok(GetItemOutput::default()));

        let repo = CacheRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo
            .get_item("key", DataType::FtMetadata)
            .await
            .unwrap_err();
        assert!(matches!(error, CacheRepositoryError::KeyNotFound(_)));
    }

    #[rstest]
    #[tokio::test]
    async fn get_cache_item_deserializing_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| {
                Ok(GetItemOutput {
                    item: Some(HashMap::default()),
                    ..GetItemOutput::default()
                })
            });

        let repo = CacheRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo
            .get_item("key", DataType::FtMetadata)
            .await
            .unwrap_err();
        assert!(matches!(error, CacheRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("Error deserializing record"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_cache_item(mut fixture: TestFixture) {
        let now = Utc::now();
        let key = uuid::Uuid::new_v4();

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let cache = GenericJsonCache {
                    sk: key.to_string(),
                    pk: DataType::FtMetadata,
                    data: json!({
                        "name": "tst",
                        "symbol": "tst",
                        "logo": "tst",
                        "decimals": "0000"
                    }),
                    expires_at: now.timestamp(),
                    created_at: now,
                };
                let cache_item = serde_dynamo::to_item(cache).unwrap();
                Ok(GetItemOutput {
                    item: Some(cache_item),
                    ..GetItemOutput::default()
                })
            });

        let repo = CacheRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let result = repo
            .get_item(&key.to_string(), DataType::FtMetadata)
            .await
            .unwrap();
        assert_eq!(now, result.created_at);
        assert_eq!(key.to_string(), result.sk);
    }
}
