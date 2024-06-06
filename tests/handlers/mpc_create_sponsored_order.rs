use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::{put_key, put_sponsor_address_config};
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, INVALID_H160_MESSAGE,
};
use http::StatusCode;
use model::sponsor_address_config::SponsorAddressConfigType;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const CHAIN_ID: u64 = 80002;
const DATA: &str = "0xa9059cbb000000000000000000000000497838d6b9813365ee9fd6c13f1914d508d80d0d0000000000000000000000000000000000000000000000000000000000000001";
const VALUE: &str = "0";
const DEADLINE: &str = "1807594318";
const FUNCTION_NAME: &str = "mpc_create_sponsored_order";
const RECIPIENT_CONTRACT_ADDRESS: &str = "0xfa34a7cD78c1fa100E8AA2a410D41620DbB0E195";

const KEYS_TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

const ORDERS_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);
const SPONSOR_ADDRESS_CONFIG_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/sponsor_address_config_table.json"
);

const MISSING_FIELD_MESSAGE: &str = "missing field";
const INVALID_HEX_0X_MISSING_MESSAGE: &str = "expected 0x";
const INVALID_BYTES_MESSAGE: &str = "Invalid Bytes value";
const UNSUPPORTED_MEDIA_MESSAGE: &str = "media type specified in header not supported";

use ana_tools::config_loader::ConfigLoader;

fn build_request_body(
    address: &str,
    to: &str,
    deadline: &str,
    data: &str,
    value: &str,
    chain_id: u64,
) -> Value {
    let body = json!({
        "transaction": {
            "to": to,
            "value": value,
            "deadline": deadline,
            "data": data,
            "chain_id": chain_id,
        }
    })
    .to_string();

    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "address": address
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    })
}

#[derive(Deserialize)]
pub struct Config {
    pub keys_table_name: String,
    pub order_status_table_name: String,
    pub sponsor_address_config_table_name: String,
}
pub struct LocalFixture {
    pub config: Config,
}

#[derive(Deserialize, Debug)]
pub struct CreateSignatureOrderResponse {
    pub order_id: Uuid,
}

type OrderResponse = LambdaResponse<HttpLambdaResponse<CreateSignatureOrderResponse>>;
type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

fn assert_error_response(
    response: &ErrorResponse,
    expected_status: StatusCode,
    expected_code: &str,
    expected_message: &str,
) {
    assert_eq!(expected_status, response.body.status_code);
    assert_eq!(expected_code, response.body.body.code);
    assert!(response.body.body.message.contains(expected_message));
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    // Recreate tables to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        KEYS_TABLE_DEFINITION,
        config.keys_table_name.clone(),
    )
    .await;

    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ORDERS_TABLE_DEFINITION,
        config.order_status_table_name.clone(),
    )
    .await;

    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        SPONSOR_ADDRESS_CONFIG_TABLE_DEFINITION,
        config.sponsor_address_config_table_name.clone(),
    )
    .await;

    LocalFixture { config }
}

#[rstest]
#[case::ethereum(1)]
#[case::sepolia(11155111)]
#[case::polygon(137)]
#[case::amoy(80002)]
#[tokio::test(flavor = "multi_thread")]
pub async fn sponsored_validation_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] chain_id: u64,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;
    let sponsor_table_name = &local_fixture.config.sponsor_address_config_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let _ = put_sponsor_address_config(
        &dynamodb_fixture.dynamodb_client,
        sponsor_table_name,
        chain_id,
        SponsorAddressConfigType::GasPool,
    )
    .await;

    let _ = put_sponsor_address_config(
        &dynamodb_fixture.dynamodb_client,
        sponsor_table_name,
        chain_id,
        SponsorAddressConfigType::Forwarder,
    )
    .await;

    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        RECIPIENT_CONTRACT_ADDRESS,
        DEADLINE,
        DATA,
        VALUE,
        chain_id,
    );

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::ACCEPTED, response.body.status_code);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn sponsored_validation_send_to_0x0(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;
    let sponsor_table_name = &local_fixture.config.sponsor_address_config_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let _ = put_sponsor_address_config(
        &dynamodb_fixture.dynamodb_client,
        sponsor_table_name,
        CHAIN_ID,
        SponsorAddressConfigType::GasPool,
    )
    .await;

    let _ = put_sponsor_address_config(
        &dynamodb_fixture.dynamodb_client,
        sponsor_table_name,
        CHAIN_ID,
        SponsorAddressConfigType::Forwarder,
    )
    .await;

    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        "0x0",
        DEADLINE,
        DATA,
        VALUE,
        CHAIN_ID,
    );

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::ACCEPTED, response.body.status_code);
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_missing_field(fixture: &LambdaFixture) {
    // missing deadline
    let body = json!({
        "transaction": {
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "value": VALUE,
            "data": DATA,
            "chain_id": CHAIN_ID,
        }
    })
    .to_string();

    let input = json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "address": ADDRESS_FOR_MOCK_REQUESTS
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
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        MISSING_FIELD_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_invalid_h160(fixture: &LambdaFixture) {
    let input = build_request_body(
        "INVALID",
        RECIPIENT_CONTRACT_ADDRESS,
        DEADLINE,
        DATA,
        VALUE,
        CHAIN_ID,
    );
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. {e:?}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        "address with wrong type in request path",
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_invalid_to_h160(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        "INVALID",
        DEADLINE,
        DATA,
        VALUE,
        CHAIN_ID,
    );

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_H160_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_invalid_hex(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        RECIPIENT_CONTRACT_ADDRESS,
        DEADLINE,
        "INVALID",
        VALUE,
        CHAIN_ID,
    );
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_HEX_0X_MISSING_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_invalid_hex_with_0x(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        RECIPIENT_CONTRACT_ADDRESS,
        DEADLINE,
        "0xINVALID",
        VALUE,
        CHAIN_ID,
    );
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_BYTES_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_invalid_hex_0x_missing(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        RECIPIENT_CONTRACT_ADDRESS,
        DEADLINE,
        "6406516041610651325106165165106516169610",
        VALUE,
        CHAIN_ID,
    );
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_HEX_0X_MISSING_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_unsupported_chain_id(fixture: &LambdaFixture) {
    let body = json!({
            "transaction": {
                "to": RECIPIENT_CONTRACT_ADDRESS,
                "deadline": "1807594318",
                "value": "0",
                "data": DATA,
                "chain_id": 22345,
            }
          }
    )
    .to_string();

    let input = json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "address": ADDRESS_FOR_MOCK_REQUESTS
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
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        "chain_id 22345 is not supported",
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_empty_body(fixture: &LambdaFixture) {
    let body = json!({}).to_string();

    let input = json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "address": ADDRESS_FOR_MOCK_REQUESTS
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
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        "missing field `transaction` at line 1 column 2",
    )
}

#[rstest]
#[tokio::test]
pub async fn sponsored_validation_wrong_content_type(fixture: &LambdaFixture) {
    let body = json!({
        "transaction": {
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "deadline": "DEADLINE",
            "value": VALUE,
            "data": DATA,
            "chain_id": CHAIN_ID,
        }
    })
    .to_string();

    let input = json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "text/html"
      },
      "pathParameters": {
        "address": ADDRESS_FOR_MOCK_REQUESTS
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
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_error_response(
        &response,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        UNSUPPORTED_MEDIA_MESSAGE,
    )
}
