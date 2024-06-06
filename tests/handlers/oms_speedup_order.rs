use ana_tools::config_loader::ConfigLoader;
use ethers::types::H256;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS};
use model::order::helpers::{build_signature_order, build_signature_order_with_client_id};
use model::order::{OrderState, OrderTransaction};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::{get_related_items, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};

const FUNCTION_NAME: &str = "oms_speedup_order";

const TABLE_ORDERS_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const TABLE_KEYS_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

#[derive(Deserialize)]
pub struct Config {
    pub send_transaction_to_approvers_arn: String,

    pub keys_table_name: String,

    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[derive(Deserialize, Debug)]
pub struct CreateSpeedupOrderResponse {
    pub order_id: Uuid,
}

type OrderResponse = LambdaResponse<Value>;
type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let orders_table_name = config.order_status_table_name.clone();
    let keys_table_name = config.keys_table_name.clone();

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_ORDERS_DEFINITION,
        orders_table_name,
    )
    .await;

    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_KEYS_DEFINITION,
        keys_table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_request_body(order_id: &str, max_fee_per_gas: &str) -> Value {
    let body = json!({
        "transaction" : {
            "max_fee_per_gas": max_fee_per_gas,
            "max_priority_fee_per_gas": max_fee_per_gas
        }
    })
    .to_string();

    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "order_id": order_id
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    })
}

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

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn speedup_order_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    repo_fixture: &RepoFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;
    let original_order_id = Uuid::new_v4();

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let transaction_hash = H256::random().to_string();
    let order = build_signature_order(
        original_order_id,
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_request_body(&original_order_id.to_string(), "9000000000");

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    // Check that the order was correctly created in DynamoDB
    let mut speedup_orders = get_related_items(
        &dynamodb_fixture.dynamodb_client,
        original_order_id.to_string(),
    )
    .await;

    assert_eq!(1, speedup_orders.len());

    let speedup_order = repo_fixture
        .orders_repository
        .get_order_by_id(speedup_orders.remove(0))
        .await
        .unwrap();

    let speedup_order_tx_data = speedup_order.extract_signature_data().unwrap();

    let original_order = repo_fixture
        .orders_repository
        .get_order_by_id(original_order_id.to_string())
        .await
        .unwrap();

    let original_order_tx_data = original_order.extract_signature_data().unwrap();

    match (
        speedup_order_tx_data.data.transaction,
        original_order_tx_data.data.transaction,
    ) {
        (
            OrderTransaction::Eip1559 { value, data, .. },
            OrderTransaction::Eip1559 {
                value: original_value,
                data: original_data,
                ..
            },
        ) => {
            assert_eq!(original_value, value);
            assert_eq!(original_data, data);
        }
        _ => panic!("invalid replacement transaction found"),
    }

    assert_eq!(
        StatusCode::ACCEPTED,
        response.body.get("statusCode").unwrap().as_i64().unwrap() as u16
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn multiple_speedup_order_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    repo_fixture: &RepoFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;
    let original_order_id = Uuid::new_v4();

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let transaction_hash = H256::random().to_string();
    let order = build_signature_order(
        original_order_id,
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_request_body(&original_order_id.to_string(), "9000000000");

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(
        StatusCode::ACCEPTED,
        response.body.get("statusCode").unwrap().as_i64().unwrap() as u16
    );

    let input = build_request_body(&original_order_id.to_string(), "10000000000");

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(
        StatusCode::ACCEPTED,
        response.body.get("statusCode").unwrap().as_i64().unwrap() as u16
    );

    let speedup_orders = get_related_items(
        &dynamodb_fixture.dynamodb_client,
        original_order_id.to_string(),
    )
    .await;

    assert_eq!(2, speedup_orders.len());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn speedup_order_not_exists(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;
    let original_order_id = Uuid::new_v4();

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let input = build_request_body(&original_order_id.to_string(), "9000000000");

    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_error_response(
        &response,
        StatusCode::NOT_FOUND,
        "order_not_found",
        &original_order_id.to_string(),
    )
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn speedup_order_client_not_allowed(fixture: &LambdaFixture, repo_fixture: &RepoFixture) {
    let original_order_id = Uuid::new_v4();

    let transaction_hash = H256::random().to_string();
    let order = build_signature_order_with_client_id(
        original_order_id,
        OrderState::Submitted,
        Some(transaction_hash.clone()),
        "other_client_id".to_owned(),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_request_body(&original_order_id.to_string(), "9000000000");
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_error_response(
        &response,
        StatusCode::NOT_FOUND,
        "order_not_found",
        &original_order_id.to_string(),
    );
}
