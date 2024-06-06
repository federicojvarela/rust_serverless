use ana_tools::config_loader::ConfigLoader;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{ADDRESS_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS};
use model::order::helpers::build_sponsored_order;
use model::order::{OrderState, OrderStatus, OrderType};
use mpc_signature_sm::lambda_structure::event::Event;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;

const FUNCTION_NAME: &str = "mpc_transaction_bundler";

const TABLE_CACHE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/cache_table.json");

const TABLE_ORDER_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const TABLE_KEYS_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub cache_table_name: String,
    pub send_transaction_to_approvers_arn: String,
    pub keys_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

type OrderResponse = LambdaResponse<Event<Value>>;

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let cache_table_name = config.cache_table_name.clone();
    let order_table_name = config.order_status_table_name.clone();
    let keys_table_name = config.keys_table_name.clone();

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_CACHE_DEFINITION,
        cache_table_name,
    )
    .await;

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

fn build_input() -> Value {
    json!({
        "payload": {
            "maestro_signature": "02f86583aa36a78001018255f0943efdd74dd510542ff7d7e4ac1c7039e4901f3ab10100c080a0ed3a3333026ba54f95103ce14d583d6b308e47efdbc1553cb8d47576c3cfe79ea01285de0f3b8366411a62f2c82e6c1f4e92209e9ccca492c5b9f7a6e6e1b51c4c".to_owned()
        },
        "context": {
            "order_id": ORDER_ID_FOR_MOCK_REQUESTS
        }
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn bundle_sponsored_transaction_ok(
    fixture: &LambdaFixture,
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

    let new_key_id = Uuid::new_v4();
    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        keys_table_name,
        new_key_id,
        ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
    )
    .await;

    let input = build_input();
    let order = build_sponsored_order(
        Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap(),
        OrderState::Signed,
    );
    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    // check response status is OK
    assert_eq!(StatusCode::OK, response.status);

    // check Sponsored order
    let sponsored_order_from_db: OrderStatus = repo_fixture
        .orders_repository
        .get_order_by_id(ORDER_ID_FOR_MOCK_REQUESTS.to_string())
        .await
        .expect("Sponsored order not found");

    // check Wrapped order
    let body = response.body.payload;
    let wrapped_order_id = Uuid::parse_str(body["order_id"].as_str().unwrap()).unwrap();
    let wrapped_order_from_db: OrderStatus = repo_fixture
        .orders_repository
        .get_order_by_id(wrapped_order_id.to_string())
        .await
        .expect("Wrapped order not found");

    // check Wrapped order type is Signature
    assert_eq!(OrderType::Signature, wrapped_order_from_db.order_type);

    // check Wrapped order_id is listed as replaced_by in the Sponsored Order
    assert_eq!(
        sponsored_order_from_db.replaced_by.unwrap(),
        wrapped_order_id
    );

    // check Sponsored order_id is listed as replaces in the Wrapped Order
    assert_eq!(
        wrapped_order_from_db.replaces.unwrap(),
        Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap()
    );
}
