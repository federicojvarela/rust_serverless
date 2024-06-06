use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use common::serializers::h160::h160_to_lowercase_hex_string;
use ethers::types::Address;
use model::sponsor_address_config::{SponsorAddressConfig, SponsorAddressConfigType};
use rusoto_dynamodb::{AttributeValue, DynamoDb, PutItemInput, QueryInput};
use std::collections::HashMap;

use crate::{deserialize::deserialize_from_dynamo, sponsor_address_config::SponsorAddressConfigPk};

use super::{
    SponsorAddressConfigDynamoDbResource, SponsorAddressConfigRepository,
    SponsorAddressConfigRepositoryError,
};

pub struct SponsorAddressConfigRepositoryImpl<D: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: D,
}

impl<D: DynamoDb + Sync + Send> SponsorAddressConfigRepositoryImpl<D> {
    pub fn new(table_name: String, dynamodb_client: D) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn build_query_input(
        &self,
        key: &SponsorAddressConfigPk,
    ) -> Result<QueryInput, serde_dynamo::Error> {
        let key_condition_expression = "pk = :pk".to_owned();
        let expression_attribute_values = serde_dynamo::to_item(key)?;

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            ..QueryInput::default()
        })
    }

    fn build_gas_pool_item_input(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
    ) -> Result<PutItemInput, SponsorAddressConfigRepositoryError> {
        let key = SponsorAddressConfigPk::new(
            client_id.clone(),
            chain_id,
            SponsorAddressConfigType::GasPool,
        );
        let address = h160_to_lowercase_hex_string(address);
        let sponsor_address_config_resource = SponsorAddressConfigDynamoDbResource {
            pk: key.pk,
            sk: address.clone(),
            client_id,
            chain_id,
            address_type: SponsorAddressConfigType::GasPool.as_str().to_owned(),
            address,
            forwarder_name: None,
            last_modified_at: Utc::now(),
        };
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(sponsor_address_config_resource).map_err(|e| {
                SponsorAddressConfigRepositoryError::Unknown(
                    anyhow!(e).context("Error serializing customer config sponsored address"),
                )
            })?;

        Ok(PutItemInput {
            item,
            table_name: self.table_name.clone(),
            ..PutItemInput::default()
        })
    }

    fn build_forwarder_item_input(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
        forwarder_name: String,
    ) -> Result<PutItemInput, SponsorAddressConfigRepositoryError> {
        let key = SponsorAddressConfigPk::new(
            client_id.clone(),
            chain_id,
            SponsorAddressConfigType::Forwarder,
        );
        let address = h160_to_lowercase_hex_string(address);
        let sponsor_address_config_resource = SponsorAddressConfigDynamoDbResource {
            pk: key.pk,
            sk: address.clone(),
            client_id,
            chain_id,
            address_type: SponsorAddressConfigType::Forwarder.as_str().to_owned(),
            address,
            forwarder_name: Some(forwarder_name),
            last_modified_at: Utc::now(),
        };
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(sponsor_address_config_resource).map_err(|e| {
                SponsorAddressConfigRepositoryError::Unknown(
                    anyhow!(e).context("Error serializing customer config sponsored address"),
                )
            })?;

        Ok(PutItemInput {
            item,
            table_name: self.table_name.clone(),
            ..PutItemInput::default()
        })
    }
}

#[async_trait]
impl<D: DynamoDb + Sync + Send> SponsorAddressConfigRepository
    for SponsorAddressConfigRepositoryImpl<D>
{
    async fn get_addresses(
        &self,
        client_id: String,
        chain_id: u64,
        address_type: SponsorAddressConfigType,
    ) -> Result<Vec<SponsorAddressConfig>, SponsorAddressConfigRepositoryError> {
        // A query is being used because the sort key is needed also for a normal get item
        let key = SponsorAddressConfigPk::new(client_id, chain_id, address_type.clone());
        let input = self.build_query_input(&key).map_err(|e| {
            SponsorAddressConfigRepositoryError::Unknown(
                anyhow!(e).context("Unable to build sponsor address query input"),
            )
        })?;

        let items = self
            .dynamodb_client
            .query(input)
            .await
            .map_err(|e| {
                SponsorAddressConfigRepositoryError::Unknown(
                    anyhow!(e).context("Unable to get sponsor address"),
                )
            })?
            .items
            .ok_or(SponsorAddressConfigRepositoryError::Unknown(anyhow!(
                "Unable to get sponsor address"
            )))?;

        let mut addresses: Vec<SponsorAddressConfig> = vec![];
        for item in items {
            let address: SponsorAddressConfig = deserialize_from_dynamo::<
                SponsorAddressConfigDynamoDbResource,
                SponsorAddressConfigRepositoryError,
            >(item)?
            .try_into()?;
            addresses.push(address);
        }

        Ok(addresses)
    }

    async fn put_address_gas_pool(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
    ) -> Result<(), SponsorAddressConfigRepositoryError> {
        let address_config_item = self.build_gas_pool_item_input(client_id, chain_id, address)?;

        self.dynamodb_client
            .put_item(address_config_item)
            .await
            .map_err(|e| {
                SponsorAddressConfigRepositoryError::Unknown(
                    anyhow!(e).context("Unable to put sponsor gas pool address"),
                )
            })?;

        Ok(())
    }

    async fn put_address_forwarder(
        &self,
        client_id: String,
        chain_id: u64,
        address: Address,
        forwarder_name: String,
    ) -> Result<(), SponsorAddressConfigRepositoryError> {
        let address_config_item =
            self.build_forwarder_item_input(client_id, chain_id, address, forwarder_name)?;

        self.dynamodb_client
            .put_item(address_config_item)
            .await
            .map_err(|e| {
                SponsorAddressConfigRepositoryError::Unknown(
                    anyhow!(e).context("Unable to put sponsor forwarder address"),
                )
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sponsor_address_config::SponsorAddressConfigDynamoDbResource;
    use crate::sponsor_address_config::{
        sponsor_address_config_repository_impl::SponsorAddressConfigRepositoryImpl,
        SponsorAddressConfigRepository, SponsorAddressConfigRepositoryError,
    };
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use ethers::types::Address;
    use model::sponsor_address_config::SponsorAddressConfigType;
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{PutItemOutput, QueryError, QueryOutput};
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
            table_name: "sponsor_address_config".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_addresses_config_item_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(QueryError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_addresses(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                SponsorAddressConfigType::GasPool,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            SponsorAddressConfigRepositoryError::Unknown(_)
        ));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_addresses_config_item_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(|_| {
                Ok(QueryOutput {
                    items: Some(vec![]),
                    ..QueryOutput::default()
                })
            });

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.get_addresses(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            SponsorAddressConfigType::GasPool,
        )
        .await
        .unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn get_addresses_config_default_item(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![HashMap::default()]),
                    ..QueryOutput::default()
                })
            });

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.get_addresses(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            SponsorAddressConfigType::GasPool,
        )
        .await
        .unwrap_err();
    }

    #[rstest]
    #[tokio::test]
    async fn get_addresses_config_item(mut fixture: TestFixture) {
        let now = Utc::now();
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                let address_type = SponsorAddressConfigType::GasPool.as_str().to_owned();
                let sponsor_address_config = SponsorAddressConfigDynamoDbResource {
                    pk: format!("CLIENT#{CLIENT_ID_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}#ADDRESS_TYPE#{address_type}"),
                    sk: format!("ADDRESS#{}", ADDRESS_FOR_MOCK_REQUESTS),
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    address_type: SponsorAddressConfigType::GasPool.as_str().to_owned(),
                    address: ADDRESS_FOR_MOCK_REQUESTS.to_string(),
                    forwarder_name: None,
                    last_modified_at: now,
                };
                let sponsor_address_config = serde_dynamo::to_item(sponsor_address_config).unwrap();
                Ok(QueryOutput {
                    items: Some(vec![sponsor_address_config]),
                    ..QueryOutput::default()
                })
            });

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.get_addresses(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            SponsorAddressConfigType::GasPool,
        )
        .await
        .unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn put_address_config_forwarder_ok(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_put_item()
            .once()
            .returning(move |_| Ok(PutItemOutput::default()));

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        assert!(repo
            .put_address_forwarder(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                "Forwarder 1".to_owned()
            )
            .await
            .is_ok());
    }
    #[rstest]
    #[tokio::test]
    async fn put_address_config_gas_pool_ok(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_put_item()
            .once()
            .returning(move |_| Ok(PutItemOutput::default()));

        let repo = SponsorAddressConfigRepositoryImpl::new(
            fixture.table_name.clone(),
            fixture.dynamodb_client,
        );
        assert!(repo
            .put_address_gas_pool(
                CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()
            )
            .await
            .is_ok());
    }
}
