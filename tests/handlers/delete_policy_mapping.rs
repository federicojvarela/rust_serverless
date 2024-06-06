use chrono::Utc;
use http::StatusCode;
use std::str::FromStr;

use ana_tools::config_loader::ConfigLoader;

use ethers::types::Address;
use model::address_policy_registry::AddressPolicyRegistry;
use repositories::address_policy_registry::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
};
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{get_item_from_db, put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use crate::models::http_lambda_response::HttpLambdaEmptyResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
};

const FUNCTION_NAME: &str = "delete_policy_mapping";
const CONTRACT_ADDRESS: &str = "0x000386e3f7559d9b6a2f5c46b4ad1a9587d59dc3";

#[derive(Deserialize)]
struct EmptyResponse;

type Response = LambdaResponse<HttpLambdaEmptyResponse>;

const ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let table_name = config.address_policy_registry_table_name.clone();

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION,
        table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_request() -> Value {
    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS.to_string(),
        "address": ADDRESS_FOR_MOCK_REQUESTS.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      }
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn address_policy_deletion_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.address_policy_registry_table_name;

    let input = build_request();

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        Some(Address::from_str(CONTRACT_ADDRESS).unwrap()),
    );

    // Put the policy for CONTRACT_ADDRESS address
    put_item(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.address_policy_registry_table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.clone().pk,
            sk: key.clone().sk,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: Some(ADDRESS_FOR_MOCK_REQUESTS.to_owned()),
            policy: "some_policy".to_owned(),
            created_at: Utc::now(),
        },
    )
    .await;

    // Make sure it is there
    let item: Option<AddressPolicyRegistryDynamoDbResource> =
        get_item_from_db(&dynamodb_fixture.dynamodb_client, table_name, key.clone()).await;

    assert!(item.is_some());

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
    );

    let item: Option<AddressPolicyRegistry> =
        get_item_from_db(&dynamodb_fixture.dynamodb_client, table_name, key).await;

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert!(item.is_none());
}
