use ana_tools::config_loader::ConfigLoader;
use reqwest::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{
    CLIENT_ID_FOR_MOCK_REQUESTS, MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
    MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
};
use ethers::types::{Bytes, U256};
use hex::FromHex;
use model::order::helpers::{build_signature_order, build_signature_order_with_client_id};
use model::order::OrderState;
use model::order::OrderTransaction;
use mpc_signature_sm::http::errors::orders_repository_error::ORDER_NOT_FOUND;
use mpc_signature_sm::http::errors::VALIDATION_ERROR_CODE;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::{get_related_items, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};

const FUNCTION_NAME: &str = "oms_cancel_order";
const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    let table_name = config.order_status_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_input(order_id: Uuid, body: Option<Value>) -> Value {
    if let Some(body) = body {
        json!({
          "httpMethod": "POST",
          "pathParameters": {
            "order_id": order_id.to_string()
          },
          "headers": {
            "Content-Type": "application/json"
          },
          "requestContext": {
              "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
              "httpMethod": "POST",
              "requestTimeEpoch": 1589522
          },
          "body": body.to_string(),
        })
    } else {
        json!({
          "httpMethod": "POST",
          "pathParameters": {
            "order_id": order_id.to_string()
          },
          "requestContext": {
              "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
              "httpMethod": "POST",
              "requestTimeEpoch": 1589522
          },
        })
    }
}

#[rstest]
#[case::status_received(OrderState::Received)]
#[case::status_compliance_reviewed(OrderState::ApproversReviewed)]
#[case::status_signed(OrderState::Signed)]
#[tokio::test(flavor = "multi_thread")]
pub async fn cancel_order_ok(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    #[case] order_state: OrderState,
) {
    local_fixture.await;
    let order_id = Uuid::new_v4();

    let order = build_signature_order(order_id, order_state, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(order_id, None);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_eq!(
        StatusCode::ACCEPTED,
        response.body.get("statusCode").unwrap().as_i64().unwrap() as u16
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn cancel_order_in_chain_ok(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    dynamodb_fixture: &DynamoDbFixture,
) {
    local_fixture.await;
    let order_id = Uuid::new_v4();

    let order = build_signature_order(order_id, OrderState::Submitted, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(
        order_id,
        Some(json!({
            "transaction": {
                "max_fee_per_gas": (U256::from(MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string(),
                "max_priority_fee_per_gas": (U256::from(MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string()
            }
        })),
    );

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    // Check that the order was correctly created in DynamoDB
    let mut cancel_orders =
        get_related_items(&dynamodb_fixture.dynamodb_client, order_id.to_string()).await;

    assert_eq!(1, cancel_orders.len());

    let cancel_order = repo_fixture
        .orders_repository
        .get_order_by_id(cancel_orders.remove(0))
        .await
        .unwrap();

    let cancel_order_tx_data = cancel_order.extract_signature_data().unwrap();

    match cancel_order_tx_data.data.transaction {
        OrderTransaction::Eip1559 { value, data, .. } => {
            assert_eq!(U256::from(0), value);
            assert_eq!(Bytes::from_hex("0x00").unwrap(), data);
        }
        _ => panic!("invalid replacement transaction found"),
    }

    assert_eq!(
        StatusCode::ACCEPTED,
        response.body.get("statusCode").unwrap().as_i64().unwrap() as u16
    );
}

#[rstest]
#[case::status_not_submitted(OrderState::NotSubmitted)]
#[case::status_completed(OrderState::Completed)]
#[case::status_cancelled(OrderState::Cancelled)]
#[case::status_error(OrderState::Error)]
#[case::status_replaced(OrderState::Replaced)]
#[tokio::test(flavor = "multi_thread")]
pub async fn cancel_order_unprocessable_entity(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    #[case] order_state: OrderState,
) {
    local_fixture.await;

    let order_id = Uuid::new_v4();
    let error_msg =
        "can't perform this operation because the order has reached a terminal state".to_string();
    let order = build_signature_order(order_id, order_state, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(order_id, None);
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);
    assert_eq!(VALIDATION_ERROR_CODE, response.body.body.code);
    assert_eq!(error_msg, response.body.body.message);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn cancel_order_not_found(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
) {
    local_fixture.await;

    let order_id = Uuid::new_v4();

    let input = build_input(order_id, None);
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_eq!(StatusCode::NOT_FOUND, response.body.status_code);
    assert_eq!(ORDER_NOT_FOUND, response.body.body.code);
    assert_eq!(order_id.to_string(), response.body.body.message);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn cancel_order_not_owned_not_found(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
) {
    local_fixture.await;

    let order_id = Uuid::new_v4();

    let order = build_signature_order_with_client_id(
        order_id,
        OrderState::Received,
        None,
        "other_client_id".to_owned(),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(order_id, None);
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    assert_eq!(StatusCode::NOT_FOUND, response.body.status_code);
    assert_eq!(ORDER_NOT_FOUND, response.body.body.code);
    assert_eq!(order_id.to_string(), response.body.body.message);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn multiple_cancel_order_ok(
    #[future] local_fixture: LocalFixture,
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    dynamodb_fixture: &DynamoDbFixture,
) {
    local_fixture.await;
    let order_id = Uuid::new_v4();

    let order = build_signature_order(order_id, OrderState::Submitted, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(
        order_id,
        Some(json!({
            "transaction" : {
                "max_fee_per_gas": (U256::from(MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string(),
                "max_priority_fee_per_gas": (U256::from(MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string()
            }
        })),
    );

    let input2 = build_input(
        order_id,
        Some(json!({
            "transaction" : {
                "max_fee_per_gas": (U256::from(MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string(),
                "max_priority_fee_per_gas": (U256::from(MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS) + 1000).to_string()
            }
        })),
    );

    let first_response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    let second_response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input2)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:?}"));

    // Check that the order was correctly created in DynamoDB
    let mut cancel_orders =
        get_related_items(&dynamodb_fixture.dynamodb_client, order_id.to_string()).await;

    assert_eq!(2, cancel_orders.len());

    let first_cancel_order = repo_fixture
        .orders_repository
        .get_order_by_id(cancel_orders.remove(0))
        .await
        .unwrap();

    let cancel_order_tx_data = first_cancel_order.extract_signature_data().unwrap();

    match cancel_order_tx_data.data.transaction {
        OrderTransaction::Eip1559 { value, data, .. } => {
            assert_eq!(U256::from(0), value);
            assert_eq!(Bytes::from_hex("0x00").unwrap(), data);
        }
        _ => panic!("invalid replacement transaction found"),
    }

    assert_eq!(
        StatusCode::ACCEPTED,
        first_response
            .body
            .get("statusCode")
            .unwrap()
            .as_i64()
            .unwrap() as u16
    );

    let second_cancel_order = repo_fixture
        .orders_repository
        .get_order_by_id(cancel_orders.remove(0))
        .await
        .unwrap();

    let cancel_order_tx_data = second_cancel_order.extract_signature_data().unwrap();

    match cancel_order_tx_data.data.transaction {
        OrderTransaction::Eip1559 { value, data, .. } => {
            assert_eq!(U256::from(0), value);
            assert_eq!(Bytes::from_hex("0x00").unwrap(), data);
        }
        _ => panic!("invalid replacement transaction found"),
    }

    assert_eq!(
        StatusCode::ACCEPTED,
        second_response
            .body
            .get("statusCode")
            .unwrap()
            .as_i64()
            .unwrap() as u16
    );
}
