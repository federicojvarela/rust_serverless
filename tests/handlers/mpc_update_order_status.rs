use std::sync::Arc;
use std::time::Duration;

use ana_tools::config_loader::ConfigLoader;
use chrono::{DateTime, Utc};
use http::StatusCode;
use model::order::helpers::build_sponsored_order;
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::{ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS};
use model::{
    cache::{DataType, GenericJsonCache},
    order::{
        helpers::{build_cancellation_order, build_signature_order, build_speedup_order},
        OrderData, OrderState, OrderStatus, OrderType,
    },
};
use mpc_signature_sm::result::error::ErrorFromHttpHandler;
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::cache::{CacheRepository, CacheRepositoryError};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::fixtures::dynamodb::dynamodb_fixture;
use crate::fixtures::dynamodb::DynamoDbFixture;
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;

const FUNCTION_NAME: &str = "mpc_update_order_status";
const CACHE_TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/cache_table.json");
const ORDERS_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FetchOrderResponse {
    pub order_id: Uuid,
    pub order_version: String,
    pub state: OrderState,
    pub data: OrderData<Value>,
    pub created_at: DateTime<Utc>,
    pub order_type: OrderType,
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Config {
    pub cache_table_name: String,
    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
    pub cache_repository: Arc<dyn CacheRepository>,
    pub orders_repository: Arc<dyn OrdersRepository>,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    // Recreate the tables to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        CACHE_TABLE_DEFINITION,
        config.cache_table_name.clone(),
    )
    .await;
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ORDERS_TABLE_DEFINITION,
        config.order_status_table_name.clone(),
    )
    .await;

    let cache_repository = Arc::new(CacheRepositoryImpl::new(
        config.cache_table_name.clone(),
        get_dynamodb_client(),
    )) as Arc<dyn CacheRepository>;

    let orders_repository = Arc::new(OrdersRepositoryImpl::new(
        config.order_status_table_name.clone(),
        get_dynamodb_client(),
    )) as Arc<dyn OrdersRepository>;

    LocalFixture {
        config,
        cache_repository,
        orders_repository,
    }
}

fn build_input(order_id: Uuid, new_state: OrderState, current_state: Option<OrderState>) -> Value {
    let mut input = json!({
        "payload": {
            "order_id": order_id,
            "next_state": new_state,
            "update_order_statement": {
              "assignment_pairs": {"execution_arn.#sm_name": "list_append(if_not_exists(execution_arn.#sm_name, :empty_list), :execution_id)", "#data.#transaction.#nonce": ":nonce"},
              "attribute_names": {"#sm_name": "TestSM", "#data": "data", "#transaction": "transaction", "#nonce": "nonce"},
              "attribute_values": {
                    ":execution_id": {
                    "L": [
                      {
                        "S": "some_execution_id"
                      }
                    ]
                  },
                  ":empty_list": {
                    "L": []
                  },
                  ":nonce": {
                    "S": "replacement_nonce"
                  }
              }
            }
        },
        "context": {
            "order_id": order_id
        }
    });

    if let Some(current_state) = current_state {
        input["payload"]["current_state"] = current_state.as_str().into();
    }

    input
}

#[rstest]
#[case::selected_for_signing_to_not_signed(OrderState::SelectedForSigning, OrderState::NotSigned)]
#[case::signed_to_not_submitted(OrderState::Signed, OrderState::NotSubmitted)]
#[case::received_to_cancelled(OrderState::Received, OrderState::Cancelled)]
#[case::approvers_reviewed_to_cancelled(OrderState::ApproversReviewed, OrderState::Cancelled)]
#[case::selected_for_signing_to_cancelled(OrderState::SelectedForSigning, OrderState::Cancelled)]
#[case::signed_to_cancelled(OrderState::Signed, OrderState::Cancelled)]
#[case::signed_to_submitted(OrderState::Signed, OrderState::Submitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_order_status_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), current_state, None);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    let input = build_input(order_from_db.order_id, new_state, None);
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, response.status);

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");
    assert_eq!(new_state, order_from_db.state);
}

#[rstest]
#[case::selected_for_signing_to_not_signed(OrderState::SelectedForSigning, OrderState::NotSigned)]
#[case::signed_to_not_submitted(OrderState::Signed, OrderState::NotSubmitted)]
#[case::received_to_cancelled(OrderState::Received, OrderState::Cancelled)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_order_status_and_check_cache_table_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), current_state, None);
    let order_id = order.order_id.to_string();

    // create order
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order_id.clone()});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };
    _ = local_fixture.cache_repository.set_item(cache_item).await;

    // check order
    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id.clone())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    // check order entry in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();
    assert_eq!(
        order_id,
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let input = build_input(order_from_db.order_id, new_state, None);
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, response.status);

    // check that the order entry is NOT in the cache table anymove
    let order_in_terminal_state_in_cache: Result<GenericJsonCache, CacheRepositoryError> =
        local_fixture
            .cache_repository
            .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
            .await;

    assert!(order_in_terminal_state_in_cache.is_err());

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id)
        .await
        .expect("Order not found");
    assert_eq!(new_state, order_from_db.state);
}

#[rstest]
#[case::approvers_reviewed_to_not_signed(OrderState::ApproversReviewed, OrderState::NotSigned)]
#[case::cancelled_to_cancelled(OrderState::Cancelled, OrderState::Cancelled)]
#[case::completed_to_cancelled(OrderState::Completed, OrderState::Cancelled)]
#[case::received_to_not_submitted(OrderState::Received, OrderState::NotSubmitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_order_status_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), current_state, None);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, order_from_db.state);

    let input = build_input(order_from_db.order_id, new_state, None);
    let response: LambdaResponse<ErrorFromHttpHandler> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .expect("There was an error invoking {FUNCTION_NAME}: {e:?}");

    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status);

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);
}

#[rstest]
#[case::selected_for_signing_to_error(OrderState::SelectedForSigning, OrderState::Error)]
#[case::signed_to_error(OrderState::Signed, OrderState::Error)]
#[case::submitted_to_error(OrderState::Submitted, OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_order_state_to_from_locking_state_to_error_and_check_cache_table_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), current_state, None);
    let order_id = order.order_id.to_string();

    // create order
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order_id.clone()});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    // check order
    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id.clone())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    // check order entry in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order_id,
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let input = build_input(order_from_db.order_id, new_state, Some(current_state));
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, response.status);

    // check that address lock is NOT in the cache table anymove
    let address_lock = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap_err();

    assert!(matches!(address_lock, CacheRepositoryError::KeyNotFound(_)));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id)
        .await
        .expect("Order not found");

    assert_eq!(new_state, order_from_db.state);
}

#[rstest]
#[case::received_to_error(OrderState::Received, OrderState::Error)]
#[case::approvers_reviewed_to_error(OrderState::ApproversReviewed, OrderState::Error)]
#[case::reorged_to_error(OrderState::Reorged, OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_order_state_to_from_non_locking_state_to_error_and_check_cache_table_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), current_state, None);
    let order_id = order.order_id.to_string();

    // create order
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // Create entry in the cache table. This is just for checking, these state transtitions SHOULD
    // NOT remove the address lock
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order_id.clone()});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    // check order
    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id.clone())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    // check order entry in the cache table before calling the lambda
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order_id,
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let input = build_input(order_from_db.order_id, new_state, Some(current_state));
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, response.status);

    // check order entry in the cache table AFTER calling the lambda (should be there because we do
    // not come from a locking state)
    local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id)
        .await
        .expect("Order not found");

    assert_eq!(new_state, order_from_db.state);
}

#[rstest]
#[case::selected_for_signing_to_not_signed(OrderState::SelectedForSigning, OrderState::NotSigned)]
#[case::signed_to_not_submitted(OrderState::Signed, OrderState::NotSubmitted)]
#[case::received_to_cancelled(OrderState::Received, OrderState::Cancelled)]
#[case::approvers_reviewed_to_cancelled(OrderState::ApproversReviewed, OrderState::Cancelled)]
#[case::selected_for_signing_to_cancelled(OrderState::SelectedForSigning, OrderState::Cancelled)]
#[case::signed_to_cancelled(OrderState::Signed, OrderState::Cancelled)]
#[case::signed_to_submitted(OrderState::Signed, OrderState::Submitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_cancellation_order_status_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
    let replacement_order =
        build_cancellation_order(Uuid::new_v4(), current_state, order.order_id, None, None);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");
    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order.order_id});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    local_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, replacement_order_from_db.state);

    let input = build_input(replacement_order_from_db.order_id, new_state, None);
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(new_state, replacement_order_from_db.state);

    // Check order entry still in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order.order_id.to_string(),
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );
}

#[rstest]
#[case::selected_for_signing_to_not_signed(OrderState::SelectedForSigning, OrderState::NotSigned)]
#[case::signed_to_not_submitted(OrderState::Signed, OrderState::NotSubmitted)]
#[case::received_to_cancelled(OrderState::Received, OrderState::Cancelled)]
#[case::approvers_reviewed_to_cancelled(OrderState::ApproversReviewed, OrderState::Cancelled)]
#[case::selected_for_signing_to_cancelled(OrderState::SelectedForSigning, OrderState::Cancelled)]
#[case::signed_to_cancelled(OrderState::Signed, OrderState::Cancelled)]
#[case::signed_to_submitted(OrderState::Signed, OrderState::Submitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_speedup_order_status_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
    let replacement_order =
        build_speedup_order(Uuid::new_v4(), current_state, order.order_id, Utc::now());

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");
    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order.order_id});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    local_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, replacement_order_from_db.state);

    let input = build_input(replacement_order_from_db.order_id, new_state, None);
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(new_state, replacement_order_from_db.state);

    // Check order entry still in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order.order_id.to_string(),
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );
}

#[rstest]
#[case::selected_for_signing_to_error(OrderState::SelectedForSigning, OrderState::Error)]
#[case::signed_to_error(OrderState::Signed, OrderState::Error)]
#[case::submitted_to_error(OrderState::Submitted, OrderState::Error)]
#[case::received_to_error(OrderState::Received, OrderState::Error)]
#[case::approvers_reviewed_to_error(OrderState::ApproversReviewed, OrderState::Error)]
#[case::reorged_to_error(OrderState::Reorged, OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_cancellation_order_state_to_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;

    // create order
    let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create replacement order
    let replacement_order =
        build_cancellation_order(Uuid::new_v4(), current_state, order.order_id, None, None);
    local_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({ "order_id": order.order_id });
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    // check order
    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, replacement_order_from_db.state);

    let input = build_input(replacement_order.order_id, new_state, Some(current_state));
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    // check order entry in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order.order_id.to_string(),
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(new_state, replacement_order_from_db.state);
}

#[rstest]
#[case::selected_for_signing_to_error(OrderState::SelectedForSigning, OrderState::Error)]
#[case::signed_to_error(OrderState::Signed, OrderState::Error)]
#[case::submitted_to_error(OrderState::Submitted, OrderState::Error)]
#[case::received_to_error(OrderState::Received, OrderState::Error)]
#[case::approvers_reviewed_to_error(OrderState::ApproversReviewed, OrderState::Error)]
#[case::reorged_to_error(OrderState::Reorged, OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_speedup_order_state_to_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;

    // create order
    let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create replacement order
    let replacement_order =
        build_speedup_order(Uuid::new_v4(), current_state, order.order_id, Utc::now());
    local_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({ "order_id": order.order_id });
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    // check order
    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, replacement_order_from_db.state);

    let input = build_input(replacement_order.order_id, new_state, Some(current_state));
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    // check order entry in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order.order_id.to_string(),
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let replacement_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(replacement_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(new_state, replacement_order_from_db.state);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_signature_sponsored_order_state_to_not_submitted(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    // create sponsored order
    let mut sponsored_order = build_sponsored_order(Uuid::new_v4(), OrderState::Signed);
    let mut order = build_signature_order(Uuid::new_v4(), OrderState::Signed, None);
    order.replaces = Some(sponsored_order.order_id);
    sponsored_order.replaced_by = Some(order.order_id);

    // create orders
    local_fixture
        .orders_repository
        .create_order(&sponsored_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({ "order_id": order.order_id });
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    let input = build_input(order.order_id, OrderState::NotSubmitted, None);
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);

    // check that address lock is NOT in the cache table anymove
    let address_lock = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap_err();

    assert!(matches!(address_lock, CacheRepositoryError::KeyNotFound(_)));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(OrderState::NotSubmitted, order_from_db.state);
}

#[rstest]
#[case::signed_to_error(OrderState::Signed, OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_sponsored_order_state_to_from_locking_state_to_error_and_check_cache_table_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let mut order = build_signature_order(Uuid::new_v4(), current_state, None);
    let order_id = order.order_id.to_string();

    // create sponsored order
    let mut sponsored_order = build_sponsored_order(Uuid::new_v4(), current_state);
    order.replaces = Some(sponsored_order.order_id);
    sponsored_order.replaced_by = Some(order.order_id);

    // create orders
    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    local_fixture
        .orders_repository
        .create_order(&sponsored_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    // create entry in the cache table
    let order_address_and_chain_id =
        format!("ADDRESS#{ADDRESS_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}");

    let ttl = Utc::now() + Duration::from_secs(3600);
    let order_id_data = json!({"order_id": order_id.clone()});
    let cache_item = GenericJsonCache {
        pk: DataType::AddressLock,
        sk: order_address_and_chain_id.clone(),
        data: order_id_data,
        created_at: Utc::now(),
        expires_at: ttl.timestamp(),
    };

    local_fixture
        .cache_repository
        .set_item(cache_item)
        .await
        .unwrap();

    // check order
    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id.clone())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    // check order entry in the cache table
    let order_in_cache = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap();

    assert_eq!(
        order_id,
        order_in_cache
            .data
            .get("order_id")
            .unwrap()
            .as_str()
            .unwrap()
    );

    let input = build_input(order_from_db.order_id, new_state, Some(current_state));
    let response: LambdaResponse<()> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, response.status);

    // check that address lock is NOT in the cache table anymove
    let address_lock = local_fixture
        .cache_repository
        .get_item(order_address_and_chain_id.as_str(), DataType::AddressLock)
        .await
        .unwrap_err();

    assert!(matches!(address_lock, CacheRepositoryError::KeyNotFound(_)));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order_id)
        .await
        .expect("Order not found");

    assert_eq!(new_state, order_from_db.state);

    let sponsored_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(sponsored_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(new_state, sponsored_order_from_db.state);
}

#[rstest]
#[case::approvers_reviewed_to_not_signed(OrderState::ApproversReviewed, OrderState::NotSigned)]
#[case::cancelled_to_cancelled(OrderState::Cancelled, OrderState::Cancelled)]
#[case::completed_to_cancelled(OrderState::Completed, OrderState::Cancelled)]
#[case::received_to_not_submitted(OrderState::Received, OrderState::NotSubmitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_sponsored_order_status_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] current_state: OrderState,
    #[case] new_state: OrderState,
) {
    let local_fixture = local_fixture.await;
    let mut order = build_signature_order(Uuid::new_v4(), current_state, None);
    // create sponsored order
    let mut sponsored_order = build_sponsored_order(Uuid::new_v4(), current_state);
    order.replaces = Some(sponsored_order.order_id);
    sponsored_order.replaced_by = Some(order.order_id);

    local_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    local_fixture
        .orders_repository
        .create_order(&sponsored_order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, order_from_db.state);

    let input = build_input(order_from_db.order_id, new_state, None);
    let response: LambdaResponse<ErrorFromHttpHandler> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .expect("There was an error invoking {FUNCTION_NAME}: {e:?}");

    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status);

    let order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(order.order_id.to_string())
        .await
        .expect("Order not found");
    assert_eq!(current_state, order_from_db.state);

    let sponsored_order_from_db: OrderStatus = local_fixture
        .orders_repository
        .get_order_by_id(sponsored_order.order_id.to_string())
        .await
        .expect("Order not found");

    assert_eq!(current_state, sponsored_order_from_db.state);
}
