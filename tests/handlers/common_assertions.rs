use chrono::Utc;
use reqwest::StatusCode;
use serde_json::Value;

use crate::helpers::lambda::LambdaResponse;
use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::result::error::ErrorFromHttpHandler;

pub fn assert_lambda_response_context(
    response: &LambdaResponse<Event<Value>>,
    timestamp_before_request: i64,
) {
    assert_eq!(StatusCode::OK, response.status);
    let body = &response.body;
    let order_id = body.context.order_id.to_string();
    assert_eq!(ORDER_ID_FOR_MOCK_REQUESTS, order_id);
    let timestamp_after_request: i64 = Utc::now().timestamp_millis();
    let order_timestamp = body.context.order_timestamp.timestamp_millis();
    assert!(timestamp_before_request <= order_timestamp);
    assert!(order_timestamp <= timestamp_after_request);
}

pub fn assert_error_from_http_handler(
    response: LambdaResponse<ErrorFromHttpHandler>,
    expected_error_msg_substr: &str,
) {
    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status);
    let error_message = response.body.error_message;
    assert!(
        error_message.contains(expected_error_msg_substr),
        "Actual error message was: {}",
        error_message
    );
}
