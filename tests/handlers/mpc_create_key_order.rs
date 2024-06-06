use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::lambda::LambdaResponse;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};
use common::test_tools::http::constants::CLIENT_ID_FOR_MOCK_REQUESTS;
use http::StatusCode;
use rstest::rstest;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const FUNCTION_NAME: &str = "mpc_create_key_order";

const INVALID_TYPE_MESSAGE: &str = "invalid type: integer `1`, expected a string";
const MISSING_FIELD_MESSAGE: &str = "missing field `client_user_id`";
const EMPTY_FIELD_MESSAGE: &str = "empty value `client_user_id`";
const UNSUPPORTED_MEDIA_MESSAGE: &str = "media type specified in header not supported";

#[derive(Deserialize, Debug)]
pub struct CreateKeyOrderResponse {
    pub order_id: Uuid,
}

type OrderResponse = LambdaResponse<HttpLambdaResponse<CreateKeyOrderResponse>>;
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

fn build_request(body: Value) -> Value {
    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body.to_string()
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn key_validation_ok(fixture: &LambdaFixture) {
    let body = json!({ "client_user_id": "some_client_user_id" });
    let input = build_request(body);

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::ACCEPTED, response.body.status_code);
}

#[rstest]
#[tokio::test]
pub async fn key_validation_missing_field(fixture: &LambdaFixture) {
    let body = json!({});
    let input = build_request(body);

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
pub async fn key_validation_invalid_type(fixture: &LambdaFixture) {
    let body = json!({ "client_user_id": 1 });
    let input = build_request(body);

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. {e:?}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        INVALID_TYPE_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn key_validation_empty_body(fixture: &LambdaFixture) {
    let body = json!({ "client_user_id": "" });
    let input = build_request(body);

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "validation",
        EMPTY_FIELD_MESSAGE,
    )
}

#[rstest]
#[tokio::test]
pub async fn key_validation_wrong_content_type(fixture: &LambdaFixture) {
    let body = json!({ "client_user_id": "some_client_user_id" });
    let input = json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "text/html"
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body.to_string()
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

#[rstest]
#[tokio::test]
pub async fn key_validation_missing_content_type(fixture: &LambdaFixture) {
    let body = json!({ "client_user_id": "some_client_user_id" });
    let input = json!({
      "httpMethod": "POST",
      "headers": {},
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body.to_string()
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
        "Content-Type not found in request headers",
    )
}
