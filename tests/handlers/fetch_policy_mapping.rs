use crate::config::ConfigLoader;
use chrono::Utc;
use http::StatusCode;

use repositories::address_policy_registry::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
};
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
};

use ethers::types::H160;
use std::str::FromStr;

const FUNCTION_NAME: &str = "fetch_policy_mapping";
const POLICY_NAME: &str = "some_policy";

#[derive(Deserialize)]
struct EmptyResponse;

type Response = LambdaResponse<Value>;

const ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    config: Config,
}

#[derive(Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum AddressPathParam {
    Default,

    #[serde(untagged)]
    Address(ethers::types::Address),
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

fn build_request(address: String) -> Value {
    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS.to_string(),
        "address": address
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": ""
    })
}

#[rstest]
#[case::regular_address(ADDRESS_FOR_MOCK_REQUESTS)]
#[case::default_address("default")]
#[tokio::test(flavor = "multi_thread")]
pub async fn address_policy_fetch_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[case] address: &str,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.address_policy_registry_table_name;
    let mut address_to_query: Option<H160> = None;
    let mut address_to_save: Option<String> = None;

    if !address.eq_ignore_ascii_case("default") {
        address_to_query = Some(H160::from_str(address).unwrap());
        address_to_save = Some(address.to_owned());
    };

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        address_to_query,
    );

    // Put the policy for CONTRACT_ADDRESS address
    put_item(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.pk.clone(),
            sk: key.sk.clone(),
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: address_to_save,
            policy: POLICY_NAME.to_owned(),
            created_at: Utc::now(),
        },
    )
    .await;

    let input = build_request(address.to_string());

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    // Check the response (body
    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert_eq!(response["policy"], POLICY_NAME);
}
