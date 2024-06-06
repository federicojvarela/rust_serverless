use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use ethers::types::H256;
use http::StatusCode;
use rstest::*;
use rusoto_events::{CreateEventBusRequest, EventBridge, EventBridgeClient};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
use model::order::helpers::build_signature_order;
use model::order::OrderState;
use mpc_signature_sm::publish::config::EbConfig;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;

const FUNCTION_NAME: &str = "admin_force_order_selection";
const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub aws_region: String,
    pub event_bridge_event_bus_name: String,
}

pub struct LocalFixture {
    pub config: Config,
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub event_bridge_client: EventBridgeClient,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let eb_config = ConfigLoader::load_test::<EbConfig>();

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

    let event_bridge_client = EventBridgeClient::new(eb_config.aws_region.clone());

    event_bridge_client
        .create_event_bus(CreateEventBusRequest {
            name: config.event_bridge_event_bus_name.clone(),
            ..Default::default()
        })
        .await
        .unwrap();

    LocalFixture {
        config,
        orders_repository,
        event_bridge_client,
    }
}

fn build_input() -> Value {
    json!({ "order_id": ORDER_ID_FOR_MOCK_REQUESTS })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
pub async fn admin_force_order_selection_ok(fixture: &LambdaFixture, repo_fixture: &RepoFixture) {
    let input = build_input();

    let new_order_id = Uuid::try_parse(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
    let transaction_hash = H256::random().to_string();

    let order = build_signature_order(new_order_id, OrderState::Completed, Some(transaction_hash));

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
}

#[rstest]
#[case::missing_order_id(json!({ }), "order_id")]
#[case::invalid_order_id(json!({ "order_id": "invalid uuid" }), "UUID parsing failed")]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_force_order_selection_invalid_input(
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
