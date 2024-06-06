use ana_tools::config_loader::ConfigLoader;
use chrono::Utc;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS,
    HASH_FOR_MOCK_REQUESTS, MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
    MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS, TX_HASH_ERROR_FOR_MOCK_REQUESTS,
};
use model::order::helpers::{
    build_signature_order, build_signature_order_with_client_id, signature_data, sponsored_data,
};
use model::order::{GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData};
use mpc_signature_sm::result::error::ErrorFromHttpHandler;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture as lambda_fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::handlers::common_assertions::assert_error_from_http_handler;
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;

const FUNCTION_NAME: &str = "chain_listener_update_order";

const TABLE_CACHE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/cache_table.json");

const TABLE_ORDER_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/order_status_table.json"
);

const TABLE_KEYS_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

const OTHER_HASH_FOR_MOCK_REQUESTS: &str =
    "0xf93c20b30171d10e773dc2a2d8ed59524b25baddf381b83fcc4ec40f50bedb33";

#[derive(Deserialize)]
pub struct Config {
    pub cache_table_name: String,
    pub keys_table_name: String,
    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

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

fn get_eventbridge_event(hash: String, from: String) -> Value {
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
            "hash": hash,
            "nonce": "0x52c",
            "blockHash": "0x862dc7e796ca0be9f7efae722c5963cb25cb419fef6f1a97195e4b2c96ae7b5a",
            "blockNumber": "0x105a036",
            "transactionIndex": "0xe",
            "from": from,
            "to": CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS,
            "value": "0x16345785d8a0000",
            "gasPrice": "0xa2063f7bd",
            "gas": "0x35538",
            "input": "0xb6f9de950000000000000000000000000000000000000000000407829e5b2d8f5341eb3a0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000063edae5a0d8ebb5d24d1b84acd2b3115d4231b500000000000000000000000000000000000000000000000000000000644bf9340000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000003407f39df63ca276fb1f22072b1fd06adde74ed5",
            "v": "0x1",
            "r": "0x72d5645d308f9915886a2c38d8924875eac13da45d729cc1a24a0480ce23f54",
            "s": "0x346eaf9a82d5290e60c49940c1a595e7d8f32729273069d5e492e14d1678a34f",
            "type": "0x2",
            "accessList": [],
            "maxPriorityFeePerGas": MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "maxFeePerGas": MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "chainId": "0x1"
        }
    })
}

fn get_signature_or_speedup_order(
    order_id: Uuid,
    transaction_hash: String,
    state: OrderState,
    order_type: OrderType,
    replaces: Option<Uuid>,
    replaced_by: Option<Uuid>,
) -> OrderStatus {
    assert!(
        order_type == OrderType::Signature
            || order_type == OrderType::SpeedUp
            || order_type == OrderType::Sponsored
    );

    let data = if order_type == OrderType::Sponsored {
        sponsored_data()
    } else {
        signature_data()
    };

    OrderStatus {
        order_id,
        order_version: "1".to_string(),
        order_type,
        transaction_hash: Some(transaction_hash),
        state,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            },
            data,
        },
        replaces,
        replaced_by,
        last_modified_at: Utc::now(),
        created_at: Utc::now(),
        error: None,
        policy: None,
        cancellation_requested: None,
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_ok(
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

    let order_id = Uuid::new_v4();
    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

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

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        order_id.to_string().as_str()
    );

    let updated_order = repo_fixture
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(order_id, order.order_id);
    assert_eq!(OrderState::Completed, updated_order.state);
    assert!(updated_order.last_modified_at > order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_speedup_state_ok(
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

    let order_id = Uuid::new_v4();
    let speedup_order_id = Uuid::new_v4();

    let signature_transaction_hash = OTHER_HASH_FOR_MOCK_REQUESTS.to_string();
    let signature_order = get_signature_or_speedup_order(
        order_id,
        signature_transaction_hash,
        OrderState::Submitted,
        OrderType::Signature,
        None,
        Some(speedup_order_id),
    );

    repo_fixture
        .orders_repository
        .create_order(&signature_order)
        .await
        .expect("item not inserted");

    let speedup_transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();
    let speedup_order = get_signature_or_speedup_order(
        speedup_order_id,
        speedup_transaction_hash.clone(),
        OrderState::Submitted,
        OrderType::SpeedUp,
        Some(order_id),
        None,
    );

    repo_fixture
        .orders_repository
        .create_order(&speedup_order)
        .await
        .expect("item not inserted");

    let input = get_eventbridge_event(
        speedup_transaction_hash,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        speedup_order_id.to_string().as_str()
    );

    let updated_signature_order = repo_fixture
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let updated_speedup_order = repo_fixture
        .orders_repository
        .get_order_by_id(speedup_order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(signature_order.order_id, updated_signature_order.order_id);
    assert_eq!(OrderState::Replaced, updated_signature_order.state);
    assert!(signature_order.last_modified_at < updated_signature_order.last_modified_at);

    assert_eq!(speedup_order.order_id, updated_speedup_order.order_id);
    assert_eq!(OrderState::Completed, updated_speedup_order.state);
    assert!(speedup_order.last_modified_at < updated_speedup_order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_speedup_original_mined_state_ok(
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

    let order_id = Uuid::new_v4();
    let speedup_order_id = Uuid::new_v4();

    let signature_transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();
    let signature_order = get_signature_or_speedup_order(
        order_id,
        signature_transaction_hash.clone(),
        OrderState::Submitted,
        OrderType::Signature,
        None,
        Some(speedup_order_id),
    );

    repo_fixture
        .orders_repository
        .create_order(&signature_order)
        .await
        .expect("item not inserted");

    let speedup_transaction_hash = OTHER_HASH_FOR_MOCK_REQUESTS.to_string();
    let speedup_order = get_signature_or_speedup_order(
        speedup_order_id,
        speedup_transaction_hash,
        OrderState::Submitted,
        OrderType::SpeedUp,
        Some(order_id),
        None,
    );

    repo_fixture
        .orders_repository
        .create_order(&speedup_order)
        .await
        .expect("item not inserted");

    let input = get_eventbridge_event(
        signature_transaction_hash,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        order_id.to_string().as_str()
    );

    let updated_signature_order = repo_fixture
        .orders_repository
        .get_order_by_id(signature_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let updated_speedup_order = repo_fixture
        .orders_repository
        .get_order_by_id(speedup_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(signature_order.order_id, updated_signature_order.order_id);
    assert_eq!(OrderState::Completed, updated_signature_order.state);
    assert!(signature_order.last_modified_at < updated_signature_order.last_modified_at);

    assert_eq!(speedup_order.order_id, updated_speedup_order.order_id);
    // the speedup order will be updated to DROPPED later, by txn_monitor
    assert_eq!(OrderState::Submitted, updated_speedup_order.state);
    assert_eq!(
        speedup_order.last_modified_at,
        updated_speedup_order.last_modified_at
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_error_hash_not_found(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
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

    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_error_from_http_handler(response, "Transaction hash not found");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_error_hash_duplicated(
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

    let order2 = build_signature_order(
        Uuid::new_v4(),
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order2)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_error_from_http_handler(response, "More than one submitted transaction found");
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_ok_already_completed(
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

    let order_id = Uuid::new_v4();
    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

    let order = build_signature_order(
        order_id,
        OrderState::Completed,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        order_id.to_string().as_str()
    );

    let updated_order = repo_fixture
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(order_id, order.order_id);
    assert_eq!(OrderState::Completed, updated_order.state);
    assert_eq!(updated_order.last_modified_at, order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_err_invalid_order_state(
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

    let order_id = Uuid::new_v4();
    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

    let order = build_signature_order(order_id, OrderState::Signed, Some(transaction_hash.clone()));

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_error_from_http_handler(
        response,
        "Order needs to be in SUBMITTED state but is in SIGNED state",
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_error_key_address_not_found(
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
        "test_address".to_string(),
    )
    .await;

    let order_id = Uuid::new_v4();
    let transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();

    let order = build_signature_order(
        order_id,
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let updated_order = repo_fixture
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(order_id, order.order_id);
    assert_eq!(OrderState::Submitted, updated_order.state);
    assert_eq!(updated_order.last_modified_at, order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_state_tx_status_error(
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

    let order_id = Uuid::new_v4();
    let transaction_hash = TX_HASH_ERROR_FOR_MOCK_REQUESTS.to_string();

    let order = build_signature_order(
        order_id,
        OrderState::Submitted,
        Some(transaction_hash.clone()),
    );

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = get_eventbridge_event(
        transaction_hash.clone(),
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        order_id.to_string().as_str()
    );

    let updated_order = repo_fixture
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(order_id, order.order_id);
    assert_eq!(OrderState::CompletedWithError, updated_order.state);
    assert!(updated_order.last_modified_at > order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_sponsored_state_ok(
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

    let sponsored_order_id = Uuid::new_v4();
    let wrapped_order_id = Uuid::new_v4();

    let sponsored_order = get_signature_or_speedup_order(
        sponsored_order_id,
        TX_HASH_ERROR_FOR_MOCK_REQUESTS.to_string(),
        OrderState::Submitted,
        OrderType::Sponsored,
        None,
        Some(wrapped_order_id),
    );

    repo_fixture
        .orders_repository
        .create_order(&sponsored_order)
        .await
        .expect("item not inserted");

    let wrapped_transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();
    let wrapped_order = get_signature_or_speedup_order(
        wrapped_order_id,
        wrapped_transaction_hash.clone(),
        OrderState::Submitted,
        OrderType::Signature,
        Some(sponsored_order_id),
        None,
    );

    repo_fixture
        .orders_repository
        .create_order(&wrapped_order)
        .await
        .expect("item not inserted");

    let input = get_eventbridge_event(
        wrapped_transaction_hash,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        wrapped_order_id.to_string().as_str()
    );

    let updated_sponsored_order = repo_fixture
        .orders_repository
        .get_order_by_id(sponsored_order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let updated_wrapped_order = repo_fixture
        .orders_repository
        .get_order_by_id(wrapped_order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(sponsored_order.order_id, updated_sponsored_order.order_id);
    assert_eq!(OrderState::Submitted, updated_sponsored_order.state);
    assert_eq!(
        sponsored_order.last_modified_at,
        updated_sponsored_order.last_modified_at
    );

    assert_eq!(wrapped_order.order_id, updated_wrapped_order.order_id);
    assert_eq!(OrderState::Completed, updated_wrapped_order.state);
    assert!(wrapped_order.last_modified_at < updated_wrapped_order.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn update_speedup_original_mined_and_not_submitted_state_ok(
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

    let order_id = Uuid::new_v4();
    let speedup_order_id = Uuid::new_v4();

    let signature_transaction_hash = HASH_FOR_MOCK_REQUESTS.to_string();
    let signature_order = get_signature_or_speedup_order(
        order_id,
        signature_transaction_hash.clone(),
        OrderState::Submitted,
        OrderType::Signature,
        None,
        Some(speedup_order_id),
    );

    repo_fixture
        .orders_repository
        .create_order(&signature_order)
        .await
        .expect("item not inserted");

    let speedup_transaction_hash = OTHER_HASH_FOR_MOCK_REQUESTS.to_string();
    let speedup_order = get_signature_or_speedup_order(
        speedup_order_id,
        speedup_transaction_hash,
        OrderState::NotSubmitted,
        OrderType::SpeedUp,
        Some(order_id),
        None,
    );

    repo_fixture
        .orders_repository
        .create_order(&speedup_order)
        .await
        .expect("item not inserted");

    let input = get_eventbridge_event(
        signature_transaction_hash,
        ADDRESS_FOR_MOCK_REQUESTS.to_string(),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    assert_eq!(
        response.body["order_id"].as_str().unwrap(),
        order_id.to_string().as_str()
    );

    let updated_signature_order = repo_fixture
        .orders_repository
        .get_order_by_id(signature_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let updated_speedup_order = repo_fixture
        .orders_repository
        .get_order_by_id(speedup_order.order_id.to_string())
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    assert_eq!(signature_order.order_id, updated_signature_order.order_id);
    assert_eq!(OrderState::Completed, updated_signature_order.state);
    assert!(signature_order.last_modified_at < updated_signature_order.last_modified_at);

    assert_eq!(speedup_order.order_id, updated_speedup_order.order_id);
    assert_eq!(OrderState::NotSubmitted, updated_speedup_order.state);
    assert!(speedup_order.last_modified_at >= updated_speedup_order.last_modified_at);
}
