use ethers::types::Chain;
use http::StatusCode;
use rstest::rstest;
use serde_json::{json, Value};

use common::test_tools::http::constants::CLIENT_ID_FOR_MOCK_REQUESTS;

use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::lambda::LambdaResponse;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};

const FUNCTION_NAME: &str = "mpc_get_historical_fees";

type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

fn build_request_body(chain_id: String) -> Value {
    let request = json!( {
      "httpMethod": "GET",
      "pathParameters": {
        "chain_id": chain_id,
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "GET",
          "requestTimeEpoch": 1589522
      },
    });

    request
}

fn build_request_body_with_block_count(chain_id: String, block_count: &str) -> Value {
    let request = json!( {
      "httpMethod": "GET",
      "pathParameters": {
        "chain_id": chain_id,
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "GET",
          "requestTimeEpoch": 1589522
      },
      "queryStringParameters": {"block_count": block_count},
    });
    request
}

#[rstest]
#[case::block_count_zero("0")]
#[case::block_count_one("1")]
#[case::block_count_two("2")]
#[case::block_count_three("3")]
#[case::block_count_empty("")]
#[tokio::test(flavor = "multi_thread")]
async fn mpc_get_historical_fees_ok(fixture: &LambdaFixture, #[case] block_count: &str) {
    let response: LambdaResponse<Value> = match block_count.is_empty() {
        true => fixture
            .lambda
            .invoke(
                FUNCTION_NAME,
                build_request_body((Chain::Sepolia as u64).to_string()),
            )
            .await
            .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}\n{e:?}")),
        false => fixture
            .lambda
            .invoke(
                FUNCTION_NAME,
                build_request_body_with_block_count(
                    (Chain::Sepolia as u64).to_string(),
                    block_count,
                ),
            )
            .await
            .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}\n{e:?}")),
    };

    assert_eq!(StatusCode::OK, response.status);
    let response: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    // values we are asserting against were calculated by hand, the data was taken from a real
    // call to sepolia
    // max priority fee per gas
    assert_eq!(
        response["eip1559"]["max_priority_fee_per_gas"]["min"],
        "130"
    );
    assert_eq!(
        response["eip1559"]["max_priority_fee_per_gas"]["median"],
        "1229"
    );
    assert_eq!(
        response["eip1559"]["max_priority_fee_per_gas"]["max"],
        "1070952597091"
    );

    // max fee per gas
    assert_eq!(response["eip1559"]["max_fee_per_gas"]["min"], "255");
    assert_eq!(response["eip1559"]["max_fee_per_gas"]["median"], "1354");
    assert_eq!(
        response["eip1559"]["max_fee_per_gas"]["max"],
        "1070952597216"
    );

    // legacy gas price
    assert_eq!(response["legacy"]["gas_price"]["min"], "255");
    assert_eq!(response["legacy"]["gas_price"]["median"], "1354");
    assert_eq!(response["legacy"]["gas_price"]["max"], "1070952597216");
}

#[rstest]
#[case::missing_chain_id("", "chain_id with wrong type in request path")]
#[case::invalid_chain_id("0", "chain_id 0 is not supported")]
#[tokio::test(flavor = "multi_thread")]
async fn mpc_get_historical_fees_bad_chain_id(
    fixture: &LambdaFixture,
    #[case] chain_id: &str,
    #[case] expected_message: String,
) {
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, build_request_body(chain_id.to_string()))
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}\n{e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);

    assert_eq!("validation", response.body.body.code);
    assert_eq!(expected_message, response.body.body.message);
}
