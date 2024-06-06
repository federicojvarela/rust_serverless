use ana_tools::config_loader::ConfigLoader;
use chrono::{Duration, Utc};
use ethers::types::H256;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
use model::order::helpers::{build_signature_order_with_client_id, build_sponsored_order};
use model::order::OrderState;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture as lambda_fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;

const FUNCTION_NAME: &str = "tx_monitor";

const TABLE_ORDER_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const TABLE_KEYS_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub keys_table_name: String,
    pub last_modified_threshold: i64,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let order_table_name = config.order_status_table_name.clone();
    let keys_table_name = config.keys_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_ORDER_DEFINITION,
        order_table_name,
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

// this isn't required anymore
fn get_eventbridge_event() -> Value {
    json!(
    {
        "detail": {}
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
pub async fn tx_monitor_update_state_ok(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    repo_fixture: &RepoFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let keys_table_name = &local_fixture.config.keys_table_name;

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    )
    .await;

    let mut orders = vec![];
    for _ in 0..10 {
        let order_id = Uuid::new_v4();
        let transaction_hash = format!("{:?}", H256::random());
        let mut order = build_signature_order_with_client_id(
            order_id,
            OrderState::Submitted,
            Some(transaction_hash.clone()),
            new_key_id.to_string(),
        );
        order.last_modified_at = Utc::now() - Duration::minutes(120);

        repo_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

        orders.push(order);
    }

    let sponsored_order = build_sponsored_order(Uuid::new_v4(), OrderState::Submitted);
    repo_fixture
        .orders_repository
        .create_order(&sponsored_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event();
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    for order in orders {
        let updated_order = repo_fixture
            .orders_repository
            .get_order_by_id(order.order_id.to_string())
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

        assert_eq!(order.order_id, updated_order.order_id);
        assert_eq!(OrderState::Dropped, updated_order.state);
    }

    let updated_sponsored_order = repo_fixture
        .orders_repository
        .get_order_by_id(sponsored_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(sponsored_order.order_id, updated_sponsored_order.order_id);
    assert_eq!(OrderState::Submitted, updated_sponsored_order.state);
}
