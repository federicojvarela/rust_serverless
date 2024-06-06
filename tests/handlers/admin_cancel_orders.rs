use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use http::StatusCode;
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
use model::order::helpers::build_signature_order;
use model::order::OrderState;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;

const FUNCTION_NAME: &str = "admin_cancel_orders";
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

    let table_name = config.order_status_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name,
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

fn build_input(order_ids: Vec<Uuid>) -> Value {
    json!({ "order_ids": order_ids })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_cancel_orders_empty_orders_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let input = json!({"order_ids" : []});

    let _local_fixture = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert!(body["data"].as_array().unwrap().is_empty());
    assert!(body["errors"].as_array().unwrap().is_empty());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_cancel_orders_missing_order_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let order_id = Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
    let input = build_input(vec![order_id]);

    let _local_fixture = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert!(body["data"].as_array().unwrap().is_empty());
    assert_eq!(
        body["errors"]
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_str()
            .unwrap(),
        order_id.to_string().as_str()
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_cancel_orders_existing_order_ok(
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
) {
    let order_id = Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
    let input = build_input(vec![order_id]);

    let order = build_signature_order(order_id, OrderState::ApproversReviewed, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert_eq!(
        body["data"]
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_str()
            .unwrap(),
        order_id.to_string().as_str()
    );
    assert!(body["errors"].as_array().unwrap().is_empty());
}
