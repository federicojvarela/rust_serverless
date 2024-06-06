use crate::{
    deserialize::deserialize_from_dynamo,
    keys::{KeyAddressGSI, KeysRepository, KeysRepositoryError, KEY_ADDRESS_INDEX_NAME},
};
use anyhow::anyhow;
use async_trait::async_trait;
use ethers::types::Address;
use model::key::Key;
use rusoto_dynamodb::{DynamoDb, QueryInput};
pub struct KeysRepositoryImpl<T: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: T,
}

impl<T: DynamoDb + Sync + Send> KeysRepositoryImpl<T> {
    pub fn new(table_name: String, dynamodb_client: T) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn create_key_by_address_query_input(
        &self,
        address: &Address,
    ) -> Result<QueryInput, serde_dynamo::Error> {
        let key_condition_expression = "address = :address".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(KeyAddressGSI {
            address: format!("{:?}", address),
        })?;

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            index_name: Some(KEY_ADDRESS_INDEX_NAME.to_owned()),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            ..QueryInput::default()
        })
    }
}

#[async_trait]
impl<T: DynamoDb + Sync + Send> KeysRepository for KeysRepositoryImpl<T> {
    async fn get_key_by_address(&self, address: Address) -> Result<Key, KeysRepositoryError> {
        let input = self
            .create_key_by_address_query_input(&address)
            .map_err(|e| {
                KeysRepositoryError::Unknown(
                    anyhow!(e).context("Error building query for key by address "),
                )
            })?;

        let result = self
            .dynamodb_client
            .query(input)
            .await
            .map_err(|e| {
                KeysRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error querying Key by address: {address:?}")),
                )
            })?
            .items
            .and_then(|mut i| i.pop())
            .ok_or_else(|| {
                KeysRepositoryError::KeyNotFound(format!("Key with address {address:?} not found"))
            })?;

        deserialize_from_dynamo(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::keys::keys_repository_impl::KeysRepositoryImpl;
    use crate::keys::{KeyAddressGSI, KeysRepository, KeysRepositoryError, KEY_ADDRESS_INDEX_NAME};
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use ethers::abi::Address;
    use mockall::predicate::eq;
    use model::key::Key;
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{AttributeValue, QueryError, QueryInput, QueryOutput};
    use std::collections::HashMap;
    use std::str::FromStr;
    use uuid::Uuid;

    struct TestFixture {
        pub table_name: String,
        pub dynamodb_client: MockDbClient,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            table_name: "keys".to_owned(),
            dynamodb_client: MockDbClient::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_key_by_address_db_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .dynamodb_client
            .expect_query()
            .with(eq(QueryInput {
                table_name: fixture.table_name.clone(),
                index_name: Some(KEY_ADDRESS_INDEX_NAME.to_owned()),
                key_condition_expression: Some("address = :address".to_owned()),
                expression_attribute_values: Some(
                    serde_dynamo::to_item(KeyAddressGSI {
                        address: format!("{:?}", address),
                    })
                    .unwrap(),
                ),
                ..QueryInput::default()
            }))
            .once()
            .returning(|_| {
                Err(RusotoError::Service(QueryError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = KeysRepositoryImpl::new(fixture.table_name, fixture.dynamodb_client);
        let error = repo.get_key_by_address(address).await.unwrap_err();
        assert!(matches!(error, KeysRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_key_by_address_not_found(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .dynamodb_client
            .expect_query()
            .with(eq(QueryInput {
                table_name: fixture.table_name.clone(),
                index_name: Some(KEY_ADDRESS_INDEX_NAME.to_owned()),
                key_condition_expression: Some("address = :address".to_owned()),
                expression_attribute_values: Some(
                    serde_dynamo::to_item(KeyAddressGSI {
                        address: format!("{:?}", address),
                    })
                    .unwrap(),
                ),
                ..QueryInput::default()
            }))
            .once()
            .returning(|_| Ok(QueryOutput::default()));

        let repo = KeysRepositoryImpl::new(fixture.table_name, fixture.dynamodb_client);
        let error = repo.get_key_by_address(address).await.unwrap_err();
        assert!(matches!(error, KeysRepositoryError::KeyNotFound(_)));
        assert!(error.to_string().contains(format!("{address:?}").as_str()));
    }

    #[rstest]
    #[tokio::test]
    async fn get_key_by_address_deserialization_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .dynamodb_client
            .expect_query()
            .with(eq(QueryInput {
                table_name: fixture.table_name.clone(),
                index_name: Some(KEY_ADDRESS_INDEX_NAME.to_owned()),
                key_condition_expression: Some("address = :address".to_owned()),
                expression_attribute_values: Some(
                    serde_dynamo::to_item(KeyAddressGSI {
                        address: format!("{:?}", address),
                    })
                    .unwrap(),
                ),
                ..QueryInput::default()
            }))
            .once()
            .returning(|_| {
                Ok(QueryOutput {
                    items: Some(vec![HashMap::default()]),
                    ..QueryOutput::default()
                })
            });

        let repo = KeysRepositoryImpl::new(fixture.table_name, fixture.dynamodb_client);
        let error = repo.get_key_by_address(address).await.unwrap_err();
        assert!(matches!(error, KeysRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("Error deserializing record"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_key_by_address(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let expected_key = Key {
            key_id: Uuid::new_v4(),
            address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            client_user_id: "some.client.user.id".to_owned(),
            created_at: Utc::now(),
            order_type: "KEY_CREATION_ORDER".to_owned(),
            order_version: "1".to_owned(),
            owning_user_id: Uuid::new_v4(),
            public_key: "some.public.key".to_owned(),
        };
        let key_record: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(&expected_key).unwrap();
        fixture
            .dynamodb_client
            .expect_query()
            .with(eq(QueryInput {
                table_name: fixture.table_name.clone(),
                index_name: Some(KEY_ADDRESS_INDEX_NAME.to_owned()),
                key_condition_expression: Some("address = :address".to_owned()),
                expression_attribute_values: Some(
                    serde_dynamo::to_item(KeyAddressGSI {
                        address: format!("{:?}", address),
                    })
                    .unwrap(),
                ),
                ..QueryInput::default()
            }))
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![key_record.clone()]),
                    ..QueryOutput::default()
                })
            });

        let repo = KeysRepositoryImpl::new(fixture.table_name, fixture.dynamodb_client);
        let result = repo.get_key_by_address(address).await.unwrap();
        assert_eq!(expected_key.key_id, result.key_id);
        assert_eq!(expected_key.address, result.address);
        assert_eq!(expected_key.client_id, result.client_id);
        assert_eq!(expected_key.client_user_id, result.client_user_id);
        assert_eq!(expected_key.created_at, result.created_at);
        assert_eq!(expected_key.order_type, result.order_type);
        assert_eq!(expected_key.order_version, result.order_version);
        assert_eq!(expected_key.owning_user_id, result.owning_user_id);
        assert_eq!(expected_key.public_key, result.public_key);
    }
}
