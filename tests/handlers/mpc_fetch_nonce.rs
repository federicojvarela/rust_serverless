use std::str::FromStr;

use ana_tools::config_loader::ConfigLoader;
use chrono::{DateTime, Utc};
use ethers::types::{H160, U256};
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
use model::order::helpers::build_signature_order;
use model::order::OrderState;
use mpc_signature_sm::lambda_structure::event::Event;

use crate::fixtures::dynamodb::dynamodb_fixture;
use crate::fixtures::dynamodb::DynamoDbFixture;
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::handlers::common_assertions::assert_lambda_response_context;
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_nonce;

const FUNCTION_NAME: &str = "mpc_fetch_nonce";
const NONCES_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/nonces_table.json"
);

const ORDERS_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub nonces_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        NONCES_TABLE_DEFINITION,
        config.nonces_table_name.clone(),
    )
    .await;

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ORDERS_TABLE_DEFINITION,
        config.order_status_table_name.clone(),
    )
    .await;

    LocalFixture { config }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_nonce_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    repo_fixture: &RepoFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let address = "0x151B381058f91cF871E7eA1eE83c45326F61e96D";
    let chain_id = 1;

    let nonce = put_nonce(
        &dynamodb_fixture.dynamodb_client,
        local_fixture.config.nonces_table_name.as_str(),
        H160::from_str(address).unwrap(),
        chain_id,
        U256::from(5),
    )
    .await;

    let order_id = Uuid::from_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();

    let order = build_signature_order(order_id, OrderState::ApproversReviewed, None);

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = json!({
        "payload": {
            "address": address,
            "chain_id": chain_id
        },
        "context": {
            "order_id": order_id
        }
    });

    let timestamp_before_request: i64 = Utc::now().timestamp_millis();

    let response: LambdaResponse<Event<Value>> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_lambda_response_context(&response, timestamp_before_request);

    let body = response.body.payload;
    assert_eq!(
        nonce.address,
        H160::from_str(body["address"].as_str().unwrap()).unwrap()
    );

    assert_eq!(
        nonce.nonce,
        U256::from_str(body["nonce"].as_str().unwrap()).unwrap()
    );

    assert_eq!(nonce.chain_id, body["chain_id"]);

    let created_at: DateTime<Utc> =
        DateTime::from_str(body["created_at"].as_str().unwrap()).unwrap();
    assert_eq!(nonce.created_at, created_at);

    let last_modified_at: DateTime<Utc> =
        DateTime::from_str(body["last_modified_at"].as_str().unwrap()).unwrap();
    assert_eq!(nonce.last_modified_at, last_modified_at);
}
