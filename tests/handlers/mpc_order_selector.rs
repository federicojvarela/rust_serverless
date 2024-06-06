use std::sync::Arc;

use chrono::{Duration, Utc};
use http::StatusCode;
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::fixtures::dynamodb::dynamodb_fixture;
use crate::fixtures::dynamodb::DynamoDbFixture;
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::build_order;
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::{
    CHAIN_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
};
use model::order::{OrderState, OrderType};
use mpc_signature_sm::lambda_structure::event::Event;

use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

const FUNCTION_NAME: &str = "mpc_order_selector";
const ORDERS_TABLE_DEFINITION: &str = include_str!(
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
        ORDERS_TABLE_DEFINITION,
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
    json!({
        "payload": {
            "key_id": Uuid::parse_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
            "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS
        },
        "context": {
            "order_id": ORDER_ID_FOR_MOCK_REQUESTS
        }
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn order_selector_with_submitted_order_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(body["order_id"].to_string(), "null");
    assert_eq!(
        body["message"].as_str().unwrap(),
        format!("A SUBMITTED order found with id {}", order.order_id)
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn order_selector_without_approvers_reviewed_order_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(body["order_id"].to_string(), "null");
    assert_eq!(
        body["message"].as_str().unwrap(),
        "APPROVERS_REVIEWED orders not found"
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn order_selector_with_one_approvers_reviewed_order_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Signature,
        Utc::now(),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"], Value::Null);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn order_selector_with_many_approvers_reviewed_order_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Signature,
        Utc::now() - Duration::days(1),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Signature,
        Utc::now() + Duration::days(1),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

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

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"].to_string(), "null");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn order_selector_with_many_selected_for_signing_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::SelectedForSigning,
        OrderType::Signature,
        Utc::now() - Duration::days(1),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::SelectedForSigning,
        OrderType::Signature,
        Utc::now() + Duration::days(1),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::SelectedForSigning,
        OrderType::Signature,
        Utc::now(),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"].to_string(), "null");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn order_selector_with_many_signed_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::Signed,
        OrderType::Signature,
        Utc::now() - Duration::days(1),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::Signed,
        OrderType::Signature,
        Utc::now() + Duration::days(1),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Signed, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"].to_string(), "null");
}

#[rstest]
#[case::speedups(OrderType::SpeedUp)]
//#[case::cancellation(OrderType::Cancellation)]
#[tokio::test(flavor = "multi_thread")]
async fn order_selector_with_approvers_reviewed_replacement_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] replacement_type: OrderType,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::ApproversReviewed,
        replacement_type,
        Utc::now() - Duration::days(1),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::ApproversReviewed,
        replacement_type,
        Utc::now() + Duration::days(1),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::ApproversReviewed, replacement_type, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"].to_string(), "null");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn order_selector_with_approvers_reviewed_replacement_orders_cancellation_is_prioritized_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let date = Utc::now();
    let order = build_order(OrderState::ApproversReviewed, OrderType::SpeedUp, date);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::ApproversReviewed, OrderType::SpeedUp, date);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::ApproversReviewed, OrderType::Cancellation, date);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        body["order_type"].as_str().unwrap(),
        OrderType::Cancellation.as_str()
    );
    assert_eq!(body["message"].to_string(), "null");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn order_selector_with_approvers_reviewed_sponsored_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Sponsored,
        Utc::now() - Duration::days(1),
    );
    let order_id = order.order_id;

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Sponsored,
        Utc::now() + Duration::days(1),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(
        OrderState::ApproversReviewed,
        OrderType::Sponsored,
        Utc::now(),
    );

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body.payload;
    assert_eq!(
        Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap(),
        order_id
    );
    assert_eq!(body["message"].to_string(), "null");
}
