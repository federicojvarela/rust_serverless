use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};
use ana_tools::config_loader::ConfigLoader;
use common::test_tools::http::constants::*;

use reqwest::StatusCode;
use rstest::fixture;
use rstest::rstest;
use rusoto_dynamodb::DynamoDbClient;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const FUNCTION_NAME: &str = "fetch_ft_balance";
const DEFAULT_U64_CHAIN_ID: &u64 = &1;

const TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

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

    LocalFixture {
        dynamodb_client: dynamodb_fixture.dynamodb_client.clone(),
    }
}

fn build_request_body(contract_address: Vec<&str>) -> Value {
    let body = json!({ "contract_addresses": contract_address }).to_string();
    let request = json!( {
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": DEFAULT_U64_CHAIN_ID.to_string(),
        "address": ADDRESS_FOR_MOCK_REQUESTS.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    });

    request
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_ft_balance_get_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(
            FUNCTION_NAME,
            build_request_body(vec![CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS]),
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert_eq!(
        response["data"][0]["contract_address"],
        CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS.to_string()
    );
    assert_eq!(
        response["data"][0]["balance"],
        "1337000000000000000000".to_string()
    );
    assert_eq!(response["data"][0]["name"], "USD Coin".to_string());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_ft_balance_get_not_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(
            FUNCTION_NAME,
            build_request_body(vec![CONTRACT_ADDRESS_FOR_FAIL_MOCK_FT_REQUESTS]),
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    let response_json: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!(response_json["code"], "server_error".to_string());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_ft_balance_get_metadata_not_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(
            FUNCTION_NAME,
            build_request_body(vec![CONTRACT_ADDRESS_FOR_FAIL_MOCK_FT_REQUESTS]),
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    let response_json: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!(response_json["code"], "server_error".to_string());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_ft_balance_get_invalid_chain_id(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let body = json!({ "contract_addresses": [CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS] }).to_string();
    let request = json!( {
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": "0",
        "address": ADDRESS_FOR_MOCK_REQUESTS.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    });

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, request)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);
    assert_eq!("validation", response.body.body.code);
    assert_eq!("chain_id 0 is not supported", response.body.body.message);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_ft_balance_wrong_content_type(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let body = json!({ "contract_addresses": [CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS] }).to_string();
    let request = json!( {
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "text/html"
      },
      "pathParameters": {
        "chain_id": "11155111",
        "address": ADDRESS_FOR_MOCK_REQUESTS.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    });

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, request)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        response.body.status_code
    );
    assert_eq!("unsupported_media_type", response.body.body.code);
    assert_eq!(
        "media type specified in header not supported",
        response.body.body.message
    );
}
