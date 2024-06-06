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

type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

const FUNCTION_NAME: &str = "fetch_nft_balance";

const TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

const CONTRACT_ADDRESS_OK_RESPONSE: &str = "0x000386e3f7559d9b6a2f5c46b4ad1a9587d59dc3";
const CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_METADATA: &str =
    "0x1919db36ca2fa2e15f9000fd9cdc2edcf863e685";
const CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_METADATA_INNER_FIELDS: &str =
    "0x4675c7e5baafbffbca748158becba61ef3b0a263";
const CONTRACT_ADDRESS_NO_RESULTS: &str = "0x763c396673f9c391dce3361a9a71c8e161388000";

const CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_CONTRACT_METADATA: &str =
    "0x4838b106fce9647bdf1e7877bf73ce8b0bad5f97";
const CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_CONTRACT_METADATA_INNER_FIELDS: &str =
    "0xdafea492d9c6733ae3d56b7ed1adb60692c98bc5";
const CONTRACT_ADDRESS_OK_RESPONSE_WITH_WRONG_ATTRIBUTE_VALUE_TYPE: &str =
    "0xdc0479cc5bba033b3e7de9f178607150b3abce1f";

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

async fn call_nft_balance(fixture: &LambdaFixture, contract_address: &str) -> Value {
    let body = json!({ "contract_addresses": vec![contract_address] }).to_string();
    let request = json!( {
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
      },
      "body": body
    });

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, request)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. Error: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);
    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    response
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_nft_balance_ok(fixture: &LambdaFixture, #[future] local_fixture: LocalFixture) {
    let _ = local_fixture.await;

    let response = call_nft_balance(fixture, CONTRACT_ADDRESS_OK_RESPONSE).await;
    assert_eq!(
        "MHgwMDAzODZlM2Y3NTU5ZDliNmEyZjVjNDZiNGF==",
        response["pagination"]["page_key"]
    );
    assert_eq!(10, response["pagination"]["page_size"]);

    let tokens = response["tokens"].as_array().unwrap();
    assert_eq!(2, tokens.len());

    let first_nft = tokens[0].clone();
    assert_eq!("26", first_nft["balance"]);
    assert_eq!(
        CONTRACT_ADDRESS_OK_RESPONSE,
        first_nft["contract_address"].as_str().unwrap()
    );
    assert_eq!("Bored Ape Nike Club", first_nft["name"]);
    assert_eq!("BANC", first_nft["symbol"]);
    assert_eq!("Ape#231", first_nft["metadata"]["name"]);
    assert_eq!("A special ape", first_nft["metadata"]["description"]);
    assert_eq!("http://some.image", first_nft["metadata"]["image"]);
    let attributes = first_nft["metadata"]["attributes"].as_array().unwrap();
    assert_eq!(1, attributes.len());
    let attribute = attributes.first().unwrap();
    assert_eq!("some value", attribute["value"]);
    assert_eq!("some trait type", attribute["trait_type"]);

    let second_nft = tokens[1].clone();
    assert_eq!("1", second_nft["balance"]);
    assert_eq!(
        CONTRACT_ADDRESS_OK_RESPONSE,
        second_nft["contract_address"].as_str().unwrap()
    );
    assert_eq!("Bored Ape Nike Club", second_nft["name"]);
    assert_eq!("BANC", second_nft["symbol"]);
    assert_eq!("Ape#240", second_nft["metadata"]["name"]);
    assert_eq!("Normal Ape", second_nft["metadata"]["description"]);
    assert_eq!("http://some.image", second_nft["metadata"]["image"]);
    let attributes = second_nft["metadata"]["attributes"].as_array().unwrap();
    assert_eq!(1, attributes.len());
    let attribute = attributes.first().unwrap();
    assert_eq!("some value", attribute["value"]);
    assert_eq!("some trait type", attribute["trait_type"]);
}

#[rstest]
#[case::object(CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_METADATA)]
#[case::inner_fields(CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_METADATA_INNER_FIELDS)]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_nft_balance_ok_when_contract_is_missing_metadata(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] contract_address: &str,
) {
    let _ = local_fixture.await;

    let response = call_nft_balance(fixture, contract_address).await;
    assert_eq!(
        "QQgwMDAzODZlM2Y3NTU5ZDliNmEyZjVjNDZiPOI==",
        response["pagination"]["page_key"]
    );
    assert_eq!(10, response["pagination"]["page_size"]);

    let tokens = response["tokens"].as_array().unwrap();
    assert_eq!(1, tokens.len());
    let nft = tokens.first().unwrap();
    assert_eq!("1", nft["balance"]);
    assert_eq!(contract_address, nft["contract_address"].as_str().unwrap());
    assert_eq!("Crypto Kitty", nft["name"]);
    assert_eq!("CK", nft["symbol"]);
    assert_eq!("Kitty#2", nft["metadata"]["name"]);
    assert_eq!("A Kitty", nft["metadata"]["description"]);
    assert_eq!("", nft["metadata"]["image"]);
    let attributes = nft["metadata"]["attributes"].as_array().unwrap();
    assert_eq!(0, attributes.len());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_nft_balance_no_results(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let response = call_nft_balance(fixture, CONTRACT_ADDRESS_NO_RESULTS).await;
    assert!(response["pagination"]["page_key"].as_str().is_none());
    assert_eq!(10, response["pagination"]["page_size"]);

    let tokens = response["tokens"].as_array().unwrap();
    assert!(tokens.is_empty());
}

#[rstest]
#[case::object(CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_CONTRACT_METADATA)]
#[case::inner_fields(CONTRACT_ADDRESS_OK_RESPONSE_WITH_MISSING_CONTRACT_METADATA_INNER_FIELDS)]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_nft_balance_ok_when_contract_is_missing_contract_metadata(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] contract_address: &str,
) {
    let _ = local_fixture.await;

    let response = call_nft_balance(fixture, contract_address).await;
    assert_eq!(
        "MHgwMDAzODZlM2Y3NTU5ZDliNmEyZjVjNDZiNGF==",
        response["pagination"]["page_key"]
    );
    assert_eq!(10, response["pagination"]["page_size"]);

    let tokens = response["tokens"].as_array().unwrap();
    assert_eq!(1, tokens.len());

    let nft = tokens[0].clone();
    assert_eq!("26", nft["balance"]);
    assert_eq!(contract_address, nft["contract_address"].as_str().unwrap());
    assert_eq!("", nft["name"]);
    assert_eq!("", nft["symbol"]);
    assert_eq!("Ape#231", nft["metadata"]["name"]);
    assert_eq!("A special ape", nft["metadata"]["description"]);
    assert_eq!("http://some.image", nft["metadata"]["image"]);
    let attributes = nft["metadata"]["attributes"].as_array().unwrap();
    assert_eq!(1, attributes.len());
    let attribute = attributes.first().unwrap();
    assert_eq!("some value", attribute["value"]);
    assert_eq!("some trait type", attribute["trait_type"]);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_nft_balance_ok_wrong_attribute_value_type(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _ = local_fixture.await;

    let response = call_nft_balance(
        fixture,
        CONTRACT_ADDRESS_OK_RESPONSE_WITH_WRONG_ATTRIBUTE_VALUE_TYPE,
    )
    .await;
    assert_eq!(
        "MHgwMDAzODZlM2Y3NTU5ZDliNmEyZjVjNDZiNGF==",
        response["pagination"]["page_key"]
    );
    assert_eq!(10, response["pagination"]["page_size"]);

    let tokens = response["tokens"].as_array().unwrap();
    assert_eq!(1, tokens.len());

    let nft = tokens.first().unwrap();
    assert_eq!("26", nft["balance"]);
    assert_eq!(
        CONTRACT_ADDRESS_OK_RESPONSE_WITH_WRONG_ATTRIBUTE_VALUE_TYPE,
        nft["contract_address"].as_str().unwrap()
    );
    assert_eq!("Bored Ape Nike Club", nft["name"]);
    assert_eq!("BANC", nft["symbol"]);
    assert_eq!("Ape#231", nft["metadata"]["name"]);
    assert_eq!("A special ape", nft["metadata"]["description"]);
    assert_eq!("http://some.image", nft["metadata"]["image"]);
    let attributes = nft["metadata"]["attributes"].as_array().unwrap();
    assert_eq!(2, attributes.len());
    for attribute in attributes {
        assert_eq!("1000", attribute["value"]);
        assert_eq!("some trait type", attribute["trait_type"]);
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn call_nft_balance_invalid_chain_id(fixture: &LambdaFixture) {
    let body = json!({ "contract_addresses": vec![CONTRACT_ADDRESS_OK_RESPONSE] }).to_string();
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
async fn call_nft_balance_wrong_content_type(fixture: &LambdaFixture) {
    let body = json!({ "contract_addresses": vec![CONTRACT_ADDRESS_OK_RESPONSE] }).to_string();
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
