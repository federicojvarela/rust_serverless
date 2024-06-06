use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};
use http::StatusCode;

use ana_tools::config_loader::ConfigLoader;

use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;
use common::test_tools::http::constants::{ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS};

const FUNCTION_NAME: &str = "mpc_create_signature_order";
const GAS: &str = "22000";
const DATA: &str = "0x00";

const TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

const ORDERS_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const CHAIN_ID: u64 = 11155111;

const INVALID_FORMAT_MESSAGE: &str = "body failed to be converted to a json object";
const UNSUPPORTED_MEDIA_MESSAGE: &str = "media type specified in header not supported";

#[derive(Deserialize)]
pub struct Config {
    pub keys_table_name: String,
    pub order_status_table_name: String,
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
    let table_name = config.keys_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name,
    )
    .await;

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ORDERS_TABLE_DEFINITION,
        config.order_status_table_name.clone(),
    )
    .await;

    LocalFixture { config }
}

fn build_request_body(address: &str, to: &str, gas: &str, data: &str, chain_id: u64) -> Value {
    let body = json!({
            "transaction": {
                "to": to,
                "gas": gas,
                "gas_price": "800000",
                "value": "100",
                "data": data,
                "chain_id": chain_id,
            }
          }
    )
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

#[rstest]
#[case::ethereum(1)]
#[case::sepolia(11155111)]
#[case::polygon(137)]
#[case::amoy(80002)]
#[tokio::test(flavor = "multi_thread")]
pub async fn signature_validation_to_h160_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] chain_id: u64,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.keys_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        GAS,
        DATA,
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
#[case::ethereum(1)]
#[case::sepolia(11155111)]
#[case::polygon(137)]
#[case::amoy(80002)]
#[tokio::test(flavor = "multi_thread")]
pub async fn signature_validation_to_string_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] chain_id: u64,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.keys_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let input = build_request_body(ADDRESS_FOR_MOCK_REQUESTS, "test.eth", GAS, DATA, chain_id);

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::ACCEPTED, response.body.status_code);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn signature_validation_send_to_0x0(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.keys_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let input = build_request_body(ADDRESS_FOR_MOCK_REQUESTS, "0x0", GAS, DATA, CHAIN_ID);

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::ACCEPTED, response.body.status_code);
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_missing_field(fixture: &LambdaFixture) {
    // missing gas_price
    let body = json!({
        "transaction": {
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "gas": GAS,
            "value": "111111",
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_h160(fixture: &LambdaFixture) {
    let input = build_request_body("INVALID", ADDRESS_FOR_MOCK_REQUESTS, GAS, DATA, CHAIN_ID);
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
pub async fn signature_validation_invalid_u256(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        "INVALID",
        DATA,
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_u256_negative(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        "-123400",
        DATA,
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_u256_floating(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        "1234.50",
        DATA,
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_hex(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        GAS,
        "INVALID",
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_hex_with_0x(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        GAS,
        "0xINVALID",
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_invalid_hex_0x_missing(fixture: &LambdaFixture) {
    let input = build_request_body(
        ADDRESS_FOR_MOCK_REQUESTS,
        ADDRESS_FOR_MOCK_REQUESTS,
        GAS,
        "6406516041610651325106165165106516169610",
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
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_missing_field_eip1559(fixture: &LambdaFixture) {
    let body = json!({
        "transaction": {
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "gas": GAS,
            "max_fee_per_gas": "300000000",
            "value": "111111",
            "data": DATA,
            "chain_id": 11155111,
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
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_FORMAT_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn signature_validation_unsupported_chain_id(fixture: &LambdaFixture) {
    let body = json!({
            "transaction": {
                "to": ADDRESS_FOR_MOCK_REQUESTS,
                "gas": GAS,
                "gas_price": "800000",
                "value": "100",
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
pub async fn signature_validation_empty_body(fixture: &LambdaFixture) {
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
pub async fn signature_validation_wrong_content_type(fixture: &LambdaFixture) {
    let body = json!({
        "transaction": {
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "gas": GAS,
            "gas_price": "800000",
            "value": "111111",
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
