use std::collections::HashMap;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use ethers::types::{Address, H160};
use model::address_policy_registry::AddressPolicyRegistry;
use rusoto_dynamodb::{
    AttributeValue, DeleteItemInput, DynamoDb, GetItemInput, PutItemInput, QueryInput,
    UpdateItemInput,
};

use crate::{
    address_policy_registry::{AddressPolicyRegistryPk, ClientIdGSI, CLIENT_ID_INDEX_NAME},
    deserialize::deserialize_from_dynamo,
};

use super::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryRepository,
    AddressPolicyRegistryRepositoryError, UpdatePolicy,
};

pub struct AddressPolicyRegistryRepositoryImpl<D: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: D,
}

impl<D: DynamoDb + Sync + Send> AddressPolicyRegistryRepositoryImpl<D> {
    pub fn new(table_name: String, dynamodb_client: D) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn build_policies_query_input(&self, client_id: String) -> Result<QueryInput, anyhow::Error> {
        let key_condition_expression = "client_id = :client_id".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(ClientIdGSI { client_id })
            .map_err(|e| anyhow!(e).context("Error building query for policies"))?;

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            index_name: Some(CLIENT_ID_INDEX_NAME.to_owned()),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            ..QueryInput::default()
        })
    }

    fn build_policy_registry_item_input(
        &self,
        policy_mapping: AddressPolicyRegistry,
    ) -> Result<PutItemInput, AddressPolicyRegistryRepositoryError> {
        let policy_registration = AddressPolicyRegistryDynamoDbResource::from(policy_mapping);
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(policy_registration)
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(
                    anyhow!(e).context("Error serializing address policy"),
                )
            })?;

        let input = PutItemInput {
            item,
            table_name: self.table_name.clone(),
            ..PutItemInput::default()
        };

        Ok(input)
    }

    fn build_update_item_input(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<H160>,
        policy: String,
    ) -> Result<UpdateItemInput, AddressPolicyRegistryRepositoryError> {
        let key = serde_dynamo::to_item(AddressPolicyRegistryPk::new(client_id, chain_id, address))
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(
                    anyhow!(e).context("Error building update policy mapping key"),
                )
            })?;

        let update_expression =
            "SET #policy = :policy, last_modified_at = :last_modified_at".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(UpdatePolicy {
            policy,
            last_modified_at: Utc::now(),
        })
        .map_err(|e| {
            AddressPolicyRegistryRepositoryError::Unknown(
                anyhow!(e).context("Error building update policy mapping expression"),
            )
        })?;

        let expression_attribute_names =
            HashMap::from([(String::from("#policy"), String::from("policy"))]);

        Ok(UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            ..Default::default()
        })
    }
}

#[async_trait]
impl<D: DynamoDb + Sync + Send> AddressPolicyRegistryRepository
    for AddressPolicyRegistryRepositoryImpl<D>
{
    async fn get_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<Address>,
    ) -> Result<Option<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError> {
        let key: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(AddressPolicyRegistryPk::new(client_id, chain_id, address))
                .map_err(|e| {
                    AddressPolicyRegistryRepositoryError::Unknown(
                        anyhow!(e).context("generate address_policy_registry key"),
                    )
                })?;

        let result = self
            .dynamodb_client
            .get_item(GetItemInput {
                key,
                table_name: self.table_name.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(
                    anyhow!(e).context("unable to get address policy"),
                )
            })?
            .item;

        if let Some(address_policy) = result {
            let address_policy: AddressPolicyRegistry = deserialize_from_dynamo::<
                AddressPolicyRegistryDynamoDbResource,
                AddressPolicyRegistryRepositoryError,
            >(address_policy)?
            .try_into()?;

            Ok(Some(address_policy))
        } else {
            Ok(None)
        }
    }

    async fn put_policy(
        &self,
        policy_mapping: AddressPolicyRegistry,
    ) -> Result<(), AddressPolicyRegistryRepositoryError> {
        let policy_registration = self.build_policy_registry_item_input(policy_mapping)?;

        self.dynamodb_client
            .put_item(policy_registration)
            .await
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(
                    anyhow!(e).context("unable to put policy registration"),
                )
            })?;

        Ok(())
    }

    async fn delete_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<Address>,
    ) -> Result<(), AddressPolicyRegistryRepositoryError> {
        let pk: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(AddressPolicyRegistryPk::new(client_id, chain_id, address))
                .map_err(|e| {
                    AddressPolicyRegistryRepositoryError::Unknown(
                        anyhow!(e).context("generate address_policy_registry key"),
                    )
                })?;

        self.dynamodb_client
            .delete_item(DeleteItemInput {
                key: pk,
                table_name: self.table_name.clone(),

                ..Default::default()
            })
            .await
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(
                    anyhow!(e).context("unable to delete policy address mapping"),
                )
            })?;
        Ok(())
    }

    async fn update_policy(
        &self,
        client_id: String,
        chain_id: u64,
        address: Option<H160>,
        policy: String,
    ) -> Result<(), AddressPolicyRegistryRepositoryError> {
        let update_input =
            self.build_update_item_input(client_id.clone(), chain_id, address, policy)?;

        self.dynamodb_client
            .update_item(update_input)
            .await
            .map_err(|e| {
                let address = address.map(|a| a.to_string()).unwrap_or_default();
                AddressPolicyRegistryRepositoryError::Unknown(anyhow!(e).context(format!(
                    "Error updating policy mapping for address: {}, chain_id: {}, client_id: {}",
                    address, chain_id, client_id
                )))
            })?;

        Ok(())
    }

    async fn get_all_policies(
        &self,
        client_id: String,
    ) -> Result<Vec<AddressPolicyRegistry>, AddressPolicyRegistryRepositoryError> {
        let input = self.build_policies_query_input(client_id.clone())?;

        let items = self
            .dynamodb_client
            .query(input)
            .await
            .map_err(|e| {
                AddressPolicyRegistryRepositoryError::Unknown(anyhow!(e).context(format!(
                    "Error querying policies with client_id: {}",
                    client_id.clone()
                )))
            })?
            .items
            .ok_or(AddressPolicyRegistryRepositoryError::PolicyNotFound(
                format!("Policy with client_id {} not found", client_id.clone()),
            ))?;

        let mut policies = Vec::with_capacity(items.len());
        for item in items {
            let policy = deserialize_from_dynamo::<
                AddressPolicyRegistryDynamoDbResource,
                AddressPolicyRegistryRepositoryError,
            >(item)?;

            policies.push(AddressPolicyRegistry::try_from(policy)?);
        }

        Ok(policies)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::str::FromStr;

    use crate::address_policy_registry::{
        address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl,
        AddressPolicyRegistryRepository, AddressPolicyRegistryRepositoryError,
    };
    use crate::address_policy_registry::{
        AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
    };
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use ethers::types::{Address, H160};
    use mockall::predicate::eq;
    use model::address_policy_registry::{AddressPolicyRegistryBuilder, AddressPolicyRegistryType};
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{
        AttributeValue, DeleteItemOutput, GetItemError, GetItemInput, GetItemOutput, PutItemOutput,
        UpdateItemError, UpdateItemOutput,
    };

    struct TestFixture {
        pub dynamodb_client: MockDbClient,
        pub table_name: String,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            dynamodb_client: MockDbClient::new(),
            table_name: "address_policy_registry".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_policy_item_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(GetItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AddressPolicyRegistryRepositoryError::Unknown(_)
        ));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_policy_item_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| Ok(GetItemOutput::default()));

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            )
            .await
            .unwrap();
        assert!(error.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn get_policy_default_item(mut fixture: TestFixture) {
        let key: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(AddressPolicyRegistryPk::new(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                None,
            ))
            .unwrap();
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .with(eq(GetItemInput {
                key,
                table_name: fixture.table_name.clone(),
                ..Default::default()
            }))
            .returning(move |_| {
                Ok(GetItemOutput {
                    item: None,
                    ..GetItemOutput::default()
                })
            });

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                None,
            )
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn get_policy_item(mut fixture: TestFixture) {
        let now = Utc::now();
        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let address_policy_registry = AddressPolicyRegistryDynamoDbResource {
                    pk: format!("CLIENT#{CLIENT_ID_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}"),
                    sk: "ADDRESS#DEFAULT".to_string(),
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    address: Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
                    policy: "Some Policy".to_string(),
                    created_at: now,
                };
                let address_policy = serde_dynamo::to_item(address_policy_registry).unwrap();
                Ok(GetItemOutput {
                    item: Some(address_policy),
                    ..GetItemOutput::default()
                })
            });

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            )
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[rstest]
    #[tokio::test]
    async fn put_policy_ok(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_put_item()
            .once()
            .returning(move |_| Ok(PutItemOutput::default()));

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );

        let mapping = AddressPolicyRegistryBuilder::new(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            "test-policy".to_string(),
        )
        .address_to(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap());
        assert!(repo.put_policy(mapping).await.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn delete_policy_ok(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_delete_item()
            .once()
            .returning(move |_| Ok(DeleteItemOutput::default()));

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        assert!(repo
            .delete_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Some(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            )
            .await
            .is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn build_policy_registry_item_input_serialize_empty_address_ok(fixture: TestFixture) {
        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );

        let mapping = AddressPolicyRegistryBuilder::new(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            "test-policy".to_string(),
        )
        .default();

        let item = repo.build_policy_registry_item_input(mapping);
        assert!(item.is_ok());
        assert!(!item.unwrap().item.contains_key("address"));
    }

    #[rstest]
    #[tokio::test]
    async fn update_policy_status_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| {
                Err(RusotoError::Service(UpdateItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .update_policy(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                None,
                "test-policy".to_string(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AddressPolicyRegistryRepositoryError::Unknown(_)
        ));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn update_policy_mapping_ok(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| Ok(UpdateItemOutput::default()));

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.update_policy(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            None,
            "test-policy".to_string(),
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn get_all_policies_items(mut fixture: TestFixture) {
        let now = Utc::now();
        // Setup the expectation for the `query` operation instead of `get_item`
        fixture.dynamodb_client.expect_query()
        .once() // Adjust as necessary for the number of times `query` is called
        .returning(move |_| {
            let address_policy_registry = AddressPolicyRegistryDynamoDbResource {
                pk: format!("CLIENT#{CLIENT_ID_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}"),
                sk: "ADDRESS#DEFAULT".to_string(),
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                address: None,
                policy: "Some Policy".to_string(),
                created_at: now,
            };
            let address_policy = serde_dynamo::to_item(address_policy_registry).unwrap();
            Ok(rusoto_dynamodb::QueryOutput {
                items: Some(vec![address_policy]),
                ..Default::default()
            })
        });

        let repo = AddressPolicyRegistryRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_all_policies(CLIENT_ID_FOR_MOCK_REQUESTS.to_string())
            .await
            .unwrap();

        result.iter().for_each(|policy| {
            assert_eq!(policy.client_id, CLIENT_ID_FOR_MOCK_REQUESTS);
            assert_eq!(policy.chain_id, CHAIN_ID_FOR_MOCK_REQUESTS);
            assert_eq!(policy.r#type, AddressPolicyRegistryType::Default);
            assert_eq!(policy.policy, "Some Policy");
        });
    }
}
