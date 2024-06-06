use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use chrono::Utc;
use rstest::{fixture, rstest};
use serde::Deserialize;
use uuid::Uuid;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::{
    CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, HASH_FOR_MOCK_REQUESTS,
    KEY_ID_FOR_MOCK_REQUESTS,
};
use model::order::helpers::{build_signature_order, signature_data};
use model::order::{GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::model::build_order;

const TABLE_ORDER_DEFINITION: &str = include_str!(
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
    let order_table_name = config.order_status_table_name.clone();

    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_ORDER_DEFINITION,
        order_table_name,
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

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn create_replacement_order_ok(
    repo_fixture: &RepoFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

    let order = build_signature_order(
        Uuid::new_v4(),
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let new_order_id = Uuid::new_v4();
    let new_order = OrderStatus {
        order_id: new_order_id,
        order_version: "1".to_string(),
        order_type: OrderType::SpeedUp,
        transaction_hash: Some(transaction_hash),
        state: OrderState::Received,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: signature_data(),
        },
        created_at: Utc::now(),
        last_modified_at: Utc::now(),
        replaced_by: Default::default(),
        replaces: Some(order.order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    local_fixture
        .orders_repository
        .create_replacement_order(&new_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let original_order = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(original_order.replaced_by, Some(new_order_id));
    assert!(original_order.last_modified_at > order.last_modified_at);

    let new_order = local_fixture
        .orders_repository
        .get_order_by_id(new_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(new_order.replaces, Some(original_order.order_id));
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn get_orders_by_key_chain_type_state_ok(#[future] local_fixture: LocalFixture) {
    let local_fixture = local_fixture.await;

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::SpeedUp, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let orders = local_fixture
        .orders_repository
        .get_orders_by_key_chain_type_state(
            KEY_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            OrderType::Signature,
            OrderState::Submitted,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].state, OrderState::Submitted);
    assert_eq!(orders[1].state, OrderState::Submitted);
    assert_eq!(orders[0].order_type, OrderType::Signature);
    assert_eq!(orders[1].order_type, OrderType::Signature);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn get_orders_by_key_chain_type_state_limit_max_ok(
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::SpeedUp, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let orders = local_fixture
        .orders_repository
        .get_orders_by_key_chain_type_state(
            KEY_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            OrderType::Signature,
            OrderState::Submitted,
            Some(2),
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].state, OrderState::Submitted);
    assert_eq!(orders[1].state, OrderState::Submitted);
    assert_eq!(orders[0].order_type, OrderType::Signature);
    assert_eq!(orders[1].order_type, OrderType::Signature);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn get_orders_by_key_chain_type_state_limit_1_ok(#[future] local_fixture: LocalFixture) {
    let local_fixture = local_fixture.await;

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Submitted, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::Signature, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order = build_order(OrderState::Error, OrderType::SpeedUp, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let orders = local_fixture
        .orders_repository
        .get_orders_by_key_chain_type_state(
            KEY_ID_FOR_MOCK_REQUESTS.to_string(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            OrderType::Signature,
            OrderState::Submitted,
            Some(1),
        )
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].state, OrderState::Submitted);
    assert_eq!(orders[0].order_type, OrderType::Signature);
}
