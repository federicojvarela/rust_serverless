use crate::deserialize::deserialize_from_dynamo;
use crate::nonces::{
    NoncePrimaryKeyDynamoDbResource, NonceUpdateDynamoDbResource, NoncesRepository,
    NoncesRepositoryError,
};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use ethers::addressbook::Address;
use ethers::types::{H256, U256};
use hex::ToHex;
use model::nonce::Nonce;
use rusoto_core::RusotoError;
use rusoto_dynamodb::{AttributeValue, DynamoDb, GetItemInput, UpdateItemError, UpdateItemInput};
use serde_dynamo::Error;
use std::collections::HashMap;

use super::NonceSetDynamoDbResource;

pub struct NoncesRepositoryImpl<D: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: D,
}

impl<D: DynamoDb + Sync + Send> NoncesRepositoryImpl<D> {
    pub fn new(table_name: String, dynamodb_client: D) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn create_get_item_input(
        &self,
        address: Address,
        chain_id: u64,
    ) -> Result<GetItemInput, Error> {
        let key: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(NoncePrimaryKeyDynamoDbResource { address, chain_id })?;

        Ok(GetItemInput {
            key,
            table_name: self.table_name.clone(),
            consistent_read: Some(true),
            ..Default::default()
        })
    }

    fn create_increment_nonce_update_input(
        &self,
        address: Address,
        next_nonce: U256,
        current_nonce: U256,
        hash: String,
        chain_id: u64,
    ) -> Result<UpdateItemInput, Error> {
        let key: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(NoncePrimaryKeyDynamoDbResource { address, chain_id })?;

        let update_expression = "SET
             created_at = if_not_exists(created_at, :created_at),
             last_modified_at = :last_modified_at,
             transaction_hash = :transaction_hash,
             nonce = :new_nonce"
            .to_owned();

        let conditional_expression =
            "nonce = :current_nonce OR attribute_not_exists(nonce)".to_owned();

        let now = Utc::now();

        let expression_attribute_values = serde_dynamo::to_item(NonceUpdateDynamoDbResource {
            new_nonce: next_nonce,
            current_nonce,
            transaction_hash: hash,
            created_at: now,
            last_modified_at: now,
        })?;

        Ok(UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            expression_attribute_values: Some(expression_attribute_values),
            condition_expression: Some(conditional_expression),
            ..Default::default()
        })
    }

    fn create_set_nonce_update_input(
        &self,
        address: Address,
        nonce: U256,
        hash: Option<H256>,
        chain_id: u64,
    ) -> Result<UpdateItemInput, Error> {
        let key: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(NoncePrimaryKeyDynamoDbResource { address, chain_id })?;

        let update_expression = "SET
             created_at = if_not_exists(created_at, :created_at),
             last_modified_at = :last_modified_at,
             transaction_hash = :transaction_hash,
             nonce = :nonce"
            .to_owned();

        let now = Utc::now();

        let expression_attribute_values = serde_dynamo::to_item(NonceSetDynamoDbResource {
            nonce,
            transaction_hash: hash.map(|h| h.encode_hex()).unwrap_or("".to_string()),
            created_at: now,
            last_modified_at: now,
        })?;

        Ok(UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            expression_attribute_values: Some(expression_attribute_values),
            ..Default::default()
        })
    }
}

#[async_trait]
impl<D: DynamoDb + Sync + Send> NoncesRepository for NoncesRepositoryImpl<D> {
    async fn get_nonce(
        &self,
        address: Address,
        chain_id: u64,
    ) -> Result<Nonce, NoncesRepositoryError> {
        let input = self.create_get_item_input(address, chain_id).map_err(|e| {
            NoncesRepositoryError::Unknown(anyhow!(e).context("Error building query for nonce"))
        })?;

        let result = self
            .dynamodb_client
            .get_item(input)
            .await
            .map_err(|e| {
                NoncesRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error querying Nonce table for address {address}")),
                )
            })?
            .item
            .ok_or_else(|| {
                NoncesRepositoryError::NonceNotFound(format!(
                    "Nonce not found for address: {address:?} and chain_id: {chain_id}"
                ))
            })?;

        deserialize_from_dynamo(result)
    }

    /// this function will increment by one the nonce store in the dynamodb table
    /// the nonce is stored as string in hex format because dynamo only support 128 bits for Number data type
    /// because this function can be called by events out of orders or multiple writers for the same address concurrently,
    /// we implemented an optimistic lock approach, using conditional expressions and catching possible
    /// ConditionalCheckFailed errors.
    /// In case of a ConditionalCheckFailed the function will call it itself in a recursive way until the update will work.
    /// Also, this function only allows updating the nonce if the new value is greater than the previous stored value.
    async fn increment_nonce(
        &self,
        address: Address,
        transaction_nonce: U256,
        hash: String,
        chain_id: u64,
    ) -> Result<(), NoncesRepositoryError> {
        // Get current nonce and default to zero if it does not exist
        let current_nonce = match self.get_nonce(address, chain_id).await {
            Ok(n) => n.nonce,
            Err(NoncesRepositoryError::NonceNotFound(_)) => U256::from(0),
            Err(e) => return Err(e),
        };

        // if the current stored nonce is greater than the transaction received nonce we can exit early.
        // this means another event with a greater nonce was saved before.
        if current_nonce > transaction_nonce {
            tracing::info!(
                address = ?address,
                chain_id = ?chain_id,
                current_nonce = ?current_nonce,
                transaction_nonce = ?transaction_nonce,
                "nonce update skipped, current nonce {} ({:#x}) is greater than transaction nonce {} ({:#x}) for address {:?}",
                current_nonce,
                current_nonce,
                transaction_nonce,
                transaction_nonce,
                address
            );
            return Ok(());
        }

        // calculate new nonce in decimal, serde will do the conversion to hex
        let next_nonce = transaction_nonce + 1;

        let input = self
            .create_increment_nonce_update_input(
                address,
                next_nonce,
                current_nonce,
                hash.clone(),
                chain_id,
            )
            .map_err(|e| {
                NoncesRepositoryError::Unknown(
                    anyhow!(e).context("Error building increment nonce update input"),
                )
            })?;

        let update = self.dynamodb_client
            .update_item(input)
            .await
            .map(|_| ())
            .map_err(|e| match e {
                RusotoError::Service(UpdateItemError::ConditionalCheckFailed(err)) => {
                    NoncesRepositoryError::ConditionalCheckFailed(err)
                }
                _ => {
                    let msg = format!("Failed to increment nonce: for address {address:?}, chain_id {chain_id}, nonce update from {current_nonce} ({:#x}) to {next_nonce} ({:#x}) for txn_hash {hash}",current_nonce, next_nonce );
                    NoncesRepositoryError::Unknown(anyhow!(e).context(msg))
                }
            });

        // if we get a ConditionalCheckFailed error, means other event saved the nonce before us.
        // We need to retry the call and repeat the validations
        if let Err(NoncesRepositoryError::ConditionalCheckFailed(_)) = update {
            tracing::info!(
                address = ?address,
                chain_id = ?chain_id,
                current_nonce = ?current_nonce,
                transaction_nonce = ?transaction_nonce,
                "ConditionalCheckFailed detected for address {:?}, chain_id {} trying to update nonce from {current_nonce} ({:#x}) to {next_nonce} ({:#x}) for txn_hash {hash} . Retrying...",
                address,
                chain_id,
                current_nonce,
                next_nonce,
            );

            self.increment_nonce(address, transaction_nonce, hash, chain_id)
                .await
        } else {
            update
        }
    }

    async fn set_nonce(
        &self,
        address: Address,
        nonce: U256,
        hash: Option<H256>,
        chain_id: u64,
    ) -> Result<(), NoncesRepositoryError> {
        let input = self
            .create_set_nonce_update_input(address, nonce, hash, chain_id)
            .map_err(|e| {
                NoncesRepositoryError::Unknown(
                    anyhow!(e).context("Error building set nonce update input"),
                )
            })?;

        self
             .dynamodb_client
             .update_item(input)
             .await
             .map_err(|e| {
                 NoncesRepositoryError::Unknown(anyhow!(e).context(format!("Failed to set nonce: from address {address:?}, chain_id {chain_id}, nonce update to {nonce} for txn_hash {hash:?}")))
             })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::nonces::{
        nonces_repository_impl::NoncesRepositoryImpl, NoncesRepository, NoncesRepositoryError,
    };
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use ethers::abi::Address;
    use ethers::types::{H160, H256, U256};
    use model::nonce::Nonce;
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{GetItemError, GetItemOutput, UpdateItemError, UpdateItemOutput};
    use std::collections::HashMap;
    use std::str::FromStr;

    struct TestFixture {
        pub dynamodb_client: MockDbClient,
        pub table_name: String,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            dynamodb_client: MockDbClient::new(),
            table_name: "nonces".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_nonce_db_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(GetItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo.get_nonce(address, chain_id).await.unwrap_err();
        assert!(matches!(error, NoncesRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_nonce_not_found(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(|_| Ok(GetItemOutput::default()));

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo.get_nonce(address, chain_id).await.unwrap_err();
        assert!(matches!(error, NoncesRepositoryError::NonceNotFound(_)));
    }

    #[rstest]
    #[tokio::test]
    async fn get_nonce_deserializing_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;

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

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo.get_nonce(address, chain_id).await.unwrap_err();
        assert!(matches!(error, NoncesRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("Error deserializing record"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_nonce(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let now = Utc::now();
        let nonce: U256 = 1.into();

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let nonce = Nonce {
                    address,
                    chain_id,
                    nonce,
                    created_at: now,
                    last_modified_at: now,
                };
                let nonce = serde_dynamo::to_item(nonce).unwrap();
                Ok(GetItemOutput {
                    item: Some(nonce),
                    ..GetItemOutput::default()
                })
            });

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let result = repo.get_nonce(address, chain_id).await.unwrap();
        assert_eq!(nonce, result.nonce);
        assert_eq!(now, result.created_at);
        assert_eq!(now, result.last_modified_at);
        assert_eq!(chain_id, result.chain_id);
        assert_eq!(address, result.address);
    }

    #[rstest]
    #[tokio::test]
    async fn increment_nonce_db_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let nonce = 1.into();
        let hash = H160::random().to_string();
        let now = Utc::now();

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let nonce = Nonce {
                    address,
                    chain_id,
                    nonce,
                    created_at: now,
                    last_modified_at: now,
                };
                let nonce = serde_dynamo::to_item(nonce).unwrap();
                Ok(GetItemOutput {
                    item: Some(nonce),
                    ..GetItemOutput::default()
                })
            });

        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(UpdateItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo
            .increment_nonce(address, nonce, hash, chain_id)
            .await
            .unwrap_err();
        assert!(matches!(error, NoncesRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn increment_nonce_conditional_update_db_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let nonce = 1.into();
        let hash = H160::random().to_string();
        let now = Utc::now();

        fixture
            .dynamodb_client
            .expect_get_item()
            .times(2)
            .returning(move |_| {
                let nonce = Nonce {
                    address,
                    chain_id,
                    nonce,
                    created_at: now,
                    last_modified_at: now,
                };
                let nonce = serde_dynamo::to_item(nonce).unwrap();
                Ok(GetItemOutput {
                    item: Some(nonce),
                    ..GetItemOutput::default()
                })
            });

        let mut set_error = true;
        fixture
            .dynamodb_client
            .expect_update_item()
            .times(2)
            .returning(move |_| {
                if set_error {
                    set_error = false;
                    Err(RusotoError::Service(
                        UpdateItemError::ConditionalCheckFailed("error".to_owned()),
                    ))
                } else {
                    Ok(UpdateItemOutput::default())
                }
            });

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        repo.increment_nonce(address, nonce, hash, chain_id)
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn increment_nonce(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let nonce = 1.into();
        let hash = H160::random().to_string();
        let now = Utc::now();

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let nonce = Nonce {
                    address,
                    chain_id,
                    nonce,
                    created_at: now,
                    last_modified_at: now,
                };
                let nonce = serde_dynamo::to_item(nonce).unwrap();
                Ok(GetItemOutput {
                    item: Some(nonce),
                    ..GetItemOutput::default()
                })
            });

        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(|_| Ok(UpdateItemOutput::default()));

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        repo.increment_nonce(address, nonce, hash, chain_id)
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn increment_nonce_skipped_lower_nonce(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let current_nonce = 10.into();
        let transaction_nonce = 2.into();
        let hash = H160::random().to_string();
        let now = Utc::now();

        fixture
            .dynamodb_client
            .expect_get_item()
            .once()
            .returning(move |_| {
                let nonce = Nonce {
                    address,
                    chain_id,
                    nonce: current_nonce,
                    created_at: now,
                    last_modified_at: now,
                };
                let nonce = serde_dynamo::to_item(nonce).unwrap();
                Ok(GetItemOutput {
                    item: Some(nonce),
                    ..GetItemOutput::default()
                })
            });

        fixture.dynamodb_client.expect_update_item().never();

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        repo.increment_nonce(address, transaction_nonce, hash, chain_id)
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn set_nonce_db_error(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let nonce = 1.into();
        let hash = Some(H256::random());

        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(UpdateItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        let error = repo
            .set_nonce(address, nonce, hash, chain_id)
            .await
            .unwrap_err();
        assert!(matches!(error, NoncesRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn set_nonce(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let nonce = 1.into();
        let hash = Some(H256::random());

        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(|_| Ok(UpdateItemOutput::default()));

        let repo = NoncesRepositoryImpl::new(fixture.table_name.clone(), fixture.dynamodb_client);
        repo.set_nonce(address, nonce, hash, chain_id)
            .await
            .expect("should succeed");
    }
}
