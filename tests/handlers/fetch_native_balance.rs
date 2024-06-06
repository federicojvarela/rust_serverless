use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};
use ana_tools::config_loader::ConfigLoader;
use common::test_tools::http::constants::*;
use ethers::types::U256;

use reqwest::StatusCode;
use rstest::fixture;
use rstest::rstest;
use rusoto_dynamodb::DynamoDbClient;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const FUNCTION_NAME: &str = "fetch_native_balance";

const TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

const ADDRESS_WITH_BALANCE: &str = "0x308044c83a7ac91e8e82ff34ccd760b5388c5729";

type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

#[derive(Deserialize)]
pub struct Config {
    pub keys_table_name: String,
}

pub struct LocalFixture {
    pub dynamodb_client: DynamoDbClient,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let table_name = config.keys_table_name.as_str();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name.to_owned(),
    )
    .await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        Uuid::new_v4(),
        ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
    )
    .await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        Uuid::new_v4(),
        ADDRESS_WITH_BALANCE.to_owned(),
    )
    .await;

    LocalFixture {
        dynamodb_client: dynamodb_fixture.dynamodb_client.clone(),
    }
}

fn build_input(address: String) -> Value {
    json!( {
      "httpMethod": "POST",
      "pathParameters": {
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS.to_string(),
        "address": address
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": "{}"
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_native_balance_get_empty_balance(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let input = build_input(ADDRESS_FOR_MOCK_REQUESTS.to_string());
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert_eq!("0", response["balance"]);
    assert_eq!(1, response["chain_id"]);
    assert_eq!("Ether", response["name"]);
    assert_eq!("ETH", response["symbol"]);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_native_balance_get_with_balance(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let input = build_input(ADDRESS_WITH_BALANCE.to_string());
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert!(!U256::from_dec_str(response["balance"].as_str().unwrap())
        .unwrap()
        .is_zero());
    assert_eq!(1, response["chain_id"]);
    assert_eq!("Ether", response["name"]);
    assert_eq!("ETH", response["symbol"]);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_native_balance_invalid_chain_id(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;
    let input = json!( {
      "httpMethod": "POST",
      "pathParameters": {
        "chain_id": "0",
        "address": ADDRESS_WITH_BALANCE
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": "{}"
    });

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);
    assert_eq!("validation", response.body.body.code);
    assert_eq!("chain_id 0 is not supported", response.body.body.message);
}
