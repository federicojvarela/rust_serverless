use std::sync::Arc;

use chrono::Utc;
use http::StatusCode;
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};

use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::{CHAIN_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS};
use model::order::{OrderState, OrderStatus, OrderSummary, OrderType};

use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::build_order;

const FUNCTION_NAME: &str = "admin_fetch_pending_orders";
const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
    pub orders_repository: Arc<dyn OrdersRepository>,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        config.order_status_table_name.clone(),
    )
    .await;

    let orders_repository = Arc::new(OrdersRepositoryImpl::new(
        config.order_status_table_name.clone(),
        get_dynamodb_client(),
    )) as Arc<dyn OrdersRepository>;

    LocalFixture {
        config,
        orders_repository,
    }
}

fn build_input() -> Value {
    json!({ "key_id": KEY_ID_FOR_MOCK_REQUESTS, "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_fetch_pending_orders_none(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;
    let input = build_input();
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert_eq!(body["orders"].as_array(), Some(&Vec::new()));
    assert_eq!(body["orders_ids"].as_array(), None);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_fetch_pending_orders_completed_only(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(OrderState::Completed, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert_eq!(body["orders"].as_array(), Some(&Vec::new()));
    assert_eq!(body["orders_ids"].as_array(), None);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_fetch_pending_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let mut created_orders: Vec<OrderStatus> = Vec::new();
    let mut created_order_ids: Vec<String> = Vec::new();

    for order_state in OrderState::PENDING_ORDER_STATES {
        let order = build_order(order_state, OrderType::Signature, Utc::now());
        local_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));
        created_orders.push(order.clone());
        created_order_ids.push(order.order_id.to_string());

        let order = build_order(order_state, OrderType::SpeedUp, Utc::now());
        local_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));
        created_orders.push(order.clone());
        created_order_ids.push(order.order_id.to_string());
    }

    let input = build_input();
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    let body = response.body;

    // check order_ids
    let order_ids_found_opt = body["order_ids"].as_array();
    assert!(order_ids_found_opt.is_some());
    let order_ids_found = order_ids_found_opt.unwrap();
    assert_eq!(created_order_ids.len(), order_ids_found.len());
    for created_order_id in created_order_ids {
        assert!(order_ids_found.contains(&serde_json::to_value(created_order_id).unwrap()));
    }
    // check orders
    let orders_found_opt = body["orders"].as_array();
    assert!(order_ids_found_opt.is_some());
    let orders_found = orders_found_opt.unwrap();

    let orders_summaries: Vec<OrderSummary> = orders_found
        .iter()
        .map(|order_val| -> OrderSummary {
            let order_val_clone = order_val.clone();
            serde_json::from_value(order_val_clone).unwrap()
        })
        .collect();

    assert_eq!(created_orders.len(), orders_summaries.len());

    for order_summary in orders_summaries {
        for order in &created_orders {
            if order_summary.order_id == order.order_id {
                assert_eq!(order_summary.order_type, order.order_type);
                assert_eq!(order_summary.state, order.state);
                assert_eq!(order_summary.created_at, order.created_at);
            }
        }
    }
}

#[rstest]
#[case::missing_key_id(json!({ "chain_id": 1 }), "key_id")]
#[case::invalid_key_id(json!({ "key_id": "invalid uuid" }), "UUID parsing failed")]
#[case::missing_chain_id(json!({ "key_id": KEY_ID_FOR_MOCK_REQUESTS }), "chain_id")]
#[case::invalid_chain_id(json!({ "key_id": KEY_ID_FOR_MOCK_REQUESTS, "chain_id": -3 }), "invalid value")]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_fetch_pending_orders_invalid_input(
    fixture: &LambdaFixture,
    #[case] input: Value,
    #[case] error_substring: &str,
) {
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status);
    let body = response.body;
    assert!(body["errorMessage"].to_string().contains(error_substring));
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_fetch_pending_orders_cancellation_requested_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Signature,
        Utc::now(),
    );
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    local_fixture
        .orders_repository
        .request_cancellation(order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order2 = build_order(
        OrderState::ApproversReviewed,
        OrderType::Signature,
        Utc::now(),
    );
    local_fixture
        .orders_repository
        .create_order(&order2)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    let body = response.body;

    // check order_ids
    let order_ids_found_opt = body["order_ids"].as_array();
    assert!(order_ids_found_opt.is_some());
    let order_ids_found = order_ids_found_opt.unwrap();
    assert_eq!(2, order_ids_found.len());

    // check orders
    let orders_found_opt = body["orders"].as_array();
    assert!(order_ids_found_opt.is_some());
    let orders_found = orders_found_opt.unwrap();

    let orders_summaries: Vec<OrderSummary> = orders_found
        .iter()
        .map(|order_val| -> OrderSummary {
            let order_val_clone = order_val.clone();
            serde_json::from_value(order_val_clone).unwrap()
        })
        .collect();

    assert_eq!(2, orders_summaries.len());

    for order_summary in &orders_summaries {
        if order_summary.order_id == order.order_id {
            assert!(order_summary.cancellation_requested);
        } else {
            assert!(!order_summary.cancellation_requested);
        }
    }
}
