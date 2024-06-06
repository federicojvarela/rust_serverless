use ana_tools::config_loader::ConfigLoader;
use ethers::types::H256;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
use model::order::helpers::build_signature_order_with_client_id;
use model::order::{OrderState, OrderStatus};
use mpc_signature_sm::result::error::ErrorFromHttpHandler;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture as lambda_fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::handlers::common_assertions::assert_error_from_http_handler;
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;

const FUNCTION_NAME: &str = "chain_listener_update_reorged_orders";

const TABLE_ORDER_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const TABLE_KEYS_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub keys_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[derive(Serialize, Clone)]
struct OrderStatusDynamoDbResourceDynamoDBKey<'a> {
    pub order_id: &'a str,
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

fn get_eventbridge_event(hashes: Vec<String>) -> Value {
    json!(
    {
        "version": "0",
        "id": "768820dd-d7cb-91c4-2d1b-3583f6cc7f5b",
        "detail-type": "EthereumTransaction",
        "source": "ana-chain-listener",
        "account": "572976003749",
        "time": "2023-04-28T16:48:03Z",
        "region": "us-west-2",
        "resources": [],
        "detail": {
            "hashes": hashes,
            "chainId": "0x1",
            "newState": "REORGED",
        }
    })
}

fn get_invalid_eventbridge_event(hashes: Vec<String>) -> Value {
    json!(
    {
        "version": "0",
        "id": "768820dd-d7cb-91c4-2d1b-3583f6cc7f5b",
        "detail-type": "EthereumTransaction",
        "source": "ana-chain-listener",
        "account": "572976003749",
        "time": "2023-04-28T16:48:03Z",
        "region": "us-west-2",
        "resources": [],
        "detail": {
            "hashes": hashes,
            "chainId": "0x1",
            "newState": "NOT_A_VALID_STATE",
        }
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_orders_state_ok(
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

    let mut orders: Vec<OrderStatus> = vec![];
    let mut transaction_hashes: Vec<String> = vec![];
    for _ in 0..10 {
        let transaction_hash = format!("{:?}", H256::random());
        let order_id = Uuid::new_v4();
        let order = build_signature_order_with_client_id(
            order_id,
            OrderState::Submitted,
            Some(transaction_hash.clone()),
            new_key_id.to_string(),
        );

        repo_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

        orders.push(order);
        transaction_hashes.push(transaction_hash);
    }

    let input = get_eventbridge_event(transaction_hashes);
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

        assert_ne!(order.state, updated_order.state);
        assert!(order.last_modified_at < updated_order.last_modified_at);
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_orders_state_completed_to_reorged(
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

    let mut orders: Vec<OrderStatus> = vec![];
    let mut transaction_hashes: Vec<String> = vec![];
    for _ in 0..10 {
        let transaction_hash = format!("{:?}", H256::random());
        let order_id = Uuid::new_v4();
        let order = build_signature_order_with_client_id(
            order_id,
            OrderState::Completed,
            Some(transaction_hash.clone()),
            new_key_id.to_string(),
        );

        repo_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

        orders.push(order);
        transaction_hashes.push(transaction_hash);
    }

    let input = get_eventbridge_event(transaction_hashes);
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
        assert_eq!(OrderState::Reorged, updated_order.state);
        assert!(order.last_modified_at < updated_order.last_modified_at);
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_orders_state_err_invalid_order_state(
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

    let mut orders: Vec<OrderStatus> = vec![];
    let mut transaction_hashes: Vec<String> = vec![];
    for _ in 0..10 {
        let transaction_hash = format!("{:?}", H256::random());
        let order_id = Uuid::new_v4();
        let order = build_signature_order_with_client_id(
            order_id,
            OrderState::Completed,
            Some(transaction_hash.clone()),
            new_key_id.to_string(),
        );

        repo_fixture
            .orders_repository
            .create_order(&order)
            .await
            .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

        orders.push(order);
        transaction_hashes.push(transaction_hash);
    }

    let input = get_invalid_eventbridge_event(transaction_hashes);
    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status);

    assert_error_from_http_handler(
        response,
        "Unknown event type Not supported OrderState variant: NOT_A_VALID_STATE",
    );
}
