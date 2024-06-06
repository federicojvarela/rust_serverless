use std::str::FromStr;

use ana_tools::config_loader::ConfigLoader;
use chrono::{DateTime, Days, Utc};
use ethers::types::H256;
use reqwest::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS};
use model::order::helpers::{
    build_signature_order, order_data_with_hash, signature_data, signature_data_with_hash,
    speedup_data,
};
use model::order::policy::{Approval, ApprovalResponse, Policy};
use model::order::{GenericOrderData, OrderStatus, SharedOrderData};
use model::order::{OrderData, OrderState, OrderType};
use mpc_signature_sm::http::errors::orders_repository_error::ORDER_NOT_FOUND;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::repo::{repo_fixture, RepoFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key_creation_order;
use crate::models::http_lambda_response::{HttpLambdaResponse, LambdaErrorResponse};

const FUNCTION_NAME: &str = "mpc_fetch_order";
const TABLE_DEFINITION: &str = include_str!(
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

type OrderResponse = LambdaResponse<HttpLambdaResponse<FetchOrderResponse>>;
type ErrorResponse = LambdaResponse<HttpLambdaResponse<LambdaErrorResponse>>;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
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

    LocalFixture { config }
}

fn build_input(order_id: Uuid) -> Value {
    json!( {
      "httpMethod": "GET",
      "pathParameters": {
        "order_id": order_id.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "GET",
          "requestTimeEpoch": 1589522
      },
      "body": "{}"
    })
}

fn build_policy(order_id: String) -> Policy {
    Policy {
        name: "SomePolicy".to_owned(),
        approvals: vec![
            Approval {
                name: "DomainAE_Approved".to_owned(),
                level: "Domain".to_owned(),
                response: Some(ApprovalResponse {
                    order_id: order_id.clone(),
                    status_reason: String::default(),
                    approval_status: 1,
                    approver_name: "DomainAE_Approved".to_owned(),
                    metadata: String::default(),
                    metadata_signature: String::default(),
                }),
            },
            Approval {
                name: "TenantAE_Rejected".to_owned(),
                level: "Tenant".to_owned(),
                response: Some(ApprovalResponse {
                    order_id: order_id.clone(),
                    status_reason: String::default(),
                    approval_status: 0,
                    approver_name: "TenantAE_Rejected".to_owned(),
                    metadata: String::default(),
                    metadata_signature: String::default(),
                }),
            },
            Approval {
                name: "DomainAE_Pending".to_owned(),
                level: "Domain".to_owned(),
                response: None,
            },
        ],
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_signature_order_ok(fixture: &LambdaFixture, repo_fixture: &RepoFixture) {
    let new_order_id = Uuid::new_v4();
    let transaction_hash = H256::random().to_string();

    let mut order = build_signature_order(
        new_order_id,
        OrderState::Completed,
        Some(transaction_hash.clone()),
    );
    order.policy = Some(build_policy(order.order_id.to_string()));

    repo_fixture
        .orders_repository
        .create_order(&order)
        .await
        .unwrap_or_else(|e| panic!("There was an error: {e:?}"));

    let input = build_input(new_order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. {e:?}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order.order_id, response.body.body.order_id);
    assert_eq!(order.order_type, response.body.body.order_type);
    assert_eq!(order.order_version, response.body.body.order_version);
    assert_eq!(order.created_at, response.body.body.created_at);
    assert_eq!(order.last_modified_at, response.body.body.last_modified_at);
    assert_eq!(order.state, response.body.body.state);

    let response_data = response.body.body.data;
    assert_eq!(
        CLIENT_ID_FOR_MOCK_REQUESTS,
        response_data.shared_data.client_id
    );
    assert_eq!(5, response_data.data.as_object().unwrap().keys().count());
    assert_eq!(transaction_hash, response_data.data["transaction_hash"]);
    assert_eq!(
        order.data.data["transaction"],
        response_data.data["transaction"]
    );
    assert_eq!(ADDRESS_FOR_MOCK_REQUESTS, response_data.data["address"]);
    assert_eq!(
        order.data.data["maestro_signature"],
        response_data.data["maestro_signature"]
    );
    assert_eq!(
        3,
        response_data.data["approvals"]
            .as_object()
            .unwrap()
            .keys()
            .count(),
    );
    assert_eq!(
        "APPROVED",
        response_data.data["approvals"]["DomainAE_Approved"]
    );
    assert_eq!(
        "REJECTED",
        response_data.data["approvals"]["TenantAE_Rejected"]
    );
    assert_eq!(
        "PENDING",
        response_data.data["approvals"]["DomainAE_Pending"]
    );
}

#[rstest]
#[case::replacement_in_received_state(OrderState::Received)]
#[case::replacement_in_signed_state(OrderState::Signed)]
#[case::replacement_in_approver_reviewed_state(OrderState::ApproversReviewed)]
#[case::replacement_in_not_submitted_state(OrderState::NotSubmitted)]
#[case::replacement_in_error_state(OrderState::Error)]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_signature_original_order_replacement_in_pre_submit_state(
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    #[case] replacement_state: OrderState,
) {
    let order_id = Uuid::new_v4();
    let replacement_order_id = Uuid::new_v4();
    let order_status = OrderStatus {
        order_id,
        order_version: "1".to_owned(),
        state: OrderState::ApproversReviewed,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: signature_data(),
        },
        created_at: Utc::now(),
        order_type: OrderType::Signature,
        last_modified_at: Utc::now(),
        replaced_by: Some(replacement_order_id),
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
    let replacement_order = OrderStatus {
        order_id: replacement_order_id,
        order_version: "1".to_owned(),
        state: replacement_state,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: speedup_data(),
        },
        created_at: Utc::now(),
        order_type: OrderType::SpeedUp,
        last_modified_at: replacement_last_modified_at,
        replaced_by: None,
        replaces: Some(order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    repo_fixture
        .orders_repository
        .create_order(&order_status)
        .await
        .expect("item not inserted");

    repo_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .expect("item not inserted");

    let input = build_input(order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order_status.order_id, response.body.body.order_id);
    assert_eq!(order_status.order_type, response.body.body.order_type);
    assert_eq!(order_status.order_version, response.body.body.order_version);
    assert_eq!(order_status.created_at, response.body.body.created_at);
    assert_eq!(
        replacement_order.last_modified_at,
        response.body.body.last_modified_at
    );
    assert_eq!(order_status.state, response.body.body.state);
    assert_data(&response, &order_status, None);
}

#[rstest]
#[case::replacement_in_submitted_state(OrderState::Submitted)]
#[case::replacement_in_completed_state(OrderState::Completed)]
#[case::replacement_in_completed_with_error_state(OrderState::CompletedWithError)]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_signature_replacement_with_original_order_id(
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    #[case] replacement_state: OrderState,
) {
    let old_transaction_hash = "26d47c86afe7482ab77835290c03ee428eb281d10cf5d479cb10636941917ef8";
    let new_transaction_hash = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    let order_id = Uuid::new_v4();
    let replacement_order_id = Uuid::new_v4();
    let order_status = OrderStatus {
        order_id,
        order_version: "1".to_owned(),
        state: OrderState::Submitted,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: signature_data_with_hash(old_transaction_hash),
        },
        created_at: Utc::now(),
        order_type: OrderType::Signature,
        last_modified_at: Utc::now(),
        replaced_by: Some(replacement_order_id),
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
    let replacement_order = OrderStatus {
        order_id: replacement_order_id,
        order_version: "1".to_owned(),
        state: replacement_state,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: order_data_with_hash(new_transaction_hash, "0x1"),
        },
        created_at: Utc::now(),
        order_type: OrderType::SpeedUp,
        last_modified_at: replacement_last_modified_at,
        replaced_by: None,
        replaces: Some(order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    repo_fixture
        .orders_repository
        .create_order(&order_status)
        .await
        .expect("item not inserted");

    repo_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .expect("item not inserted");

    let input = build_input(order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order_status.order_id, response.body.body.order_id);
    assert_eq!(order_status.order_type, response.body.body.order_type);
    assert_eq!(order_status.order_version, response.body.body.order_version);
    assert_eq!(order_status.created_at, response.body.body.created_at);
    assert_eq!(
        replacement_order.last_modified_at,
        response.body.body.last_modified_at
    );
    assert_eq!(replacement_order.state, response.body.body.state);
    // The endpoint returns the transaction hash inside "data" but in reality is placed in the root
    // in the DB
    assert_data(&response, &replacement_order, Some(new_transaction_hash));
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_signature_cancellation_with_original_order_id_and_correct_state(
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
) {
    let old_transaction_hash = "26d47c86afe7482ab77835290c03ee428eb281d10cf5d479cb10636941917ef8";
    let new_transaction_hash = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    let order_id = Uuid::new_v4();
    let replacement_order_id = Uuid::new_v4();
    let order_status = OrderStatus {
        order_id,
        order_version: "1".to_owned(),
        state: OrderState::Submitted,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: signature_data_with_hash(old_transaction_hash),
        },
        created_at: Utc::now(),
        order_type: OrderType::Signature,
        last_modified_at: Utc::now(),
        replaced_by: Some(replacement_order_id),
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
    let replacement_order = OrderStatus {
        order_id: replacement_order_id,
        order_version: "1".to_owned(),
        state: OrderState::Completed,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: order_data_with_hash(new_transaction_hash, "0x1"),
        },
        created_at: Utc::now(),
        order_type: OrderType::Cancellation,
        last_modified_at: replacement_last_modified_at,
        replaced_by: None,
        replaces: Some(order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    repo_fixture
        .orders_repository
        .create_order(&order_status)
        .await
        .expect("item not inserted");

    repo_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .expect("item not inserted");

    let input = build_input(order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order_status.order_id, response.body.body.order_id);
    assert_eq!(order_status.order_type, response.body.body.order_type);
    assert_eq!(order_status.order_version, response.body.body.order_version);
    assert_eq!(OrderState::Cancelled, response.body.body.state);
    assert_eq!(order_status.created_at, response.body.body.created_at);
    assert_eq!(
        replacement_order.last_modified_at,
        response.body.body.last_modified_at
    );
    // The endpoint returns the transaction hash inside "data" but in reality is placed in the root
    // in the DB
    assert_data(&response, &replacement_order, Some(new_transaction_hash));
}

#[rstest]
#[case::original_in_completed_state(OrderState::Completed)]
#[case::original_in_completed_state(OrderState::CompletedWithError)]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_signature_original_order_if_minted_ok(
    fixture: &LambdaFixture,
    repo_fixture: &RepoFixture,
    #[case] original_order_state: OrderState,
) {
    let order_id = Uuid::new_v4();
    let replacement_order_id = Uuid::new_v4();
    let order_status = OrderStatus {
        order_id,
        order_version: "1".to_owned(),
        state: original_order_state,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: signature_data(),
        },
        created_at: Utc::now(),
        order_type: OrderType::Signature,
        last_modified_at: Utc::now(),
        replaced_by: Some(replacement_order_id),
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
    let replacement_order = OrderStatus {
        order_id: replacement_order_id,
        order_version: "1".to_owned(),
        state: OrderState::Submitted,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: speedup_data(),
        },
        created_at: Utc::now(),
        order_type: OrderType::SpeedUp,
        last_modified_at: replacement_last_modified_at,
        replaced_by: None,
        replaces: Some(order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    repo_fixture
        .orders_repository
        .create_order(&order_status)
        .await
        .expect("item not inserted");

    repo_fixture
        .orders_repository
        .create_order(&replacement_order)
        .await
        .expect("item not inserted");

    let input = build_input(order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order_status.order_id, response.body.body.order_id);
    assert_eq!(order_status.order_type, response.body.body.order_type);
    assert_eq!(order_status.order_version, response.body.body.order_version);
    assert_eq!(order_status.created_at, response.body.body.created_at);
    assert_eq!(
        replacement_order.last_modified_at,
        response.body.body.last_modified_at
    );
    assert_eq!(order_status.state, response.body.body.state);

    assert_data(&response, &order_status, None);
}

fn assert_data(response: &OrderResponse, order_status: &OrderStatus, tx_hash: Option<&str>) {
    assert_eq!(
        response.body.body.data.data["address"],
        ADDRESS_FOR_MOCK_REQUESTS
    );

    assert_eq!(response.body.body.data.data["key_id"], Value::Null);

    assert_eq!(
        response.body.body.data.data["transaction"],
        order_status.data.data["transaction"]
    );

    if tx_hash.is_some() {
        assert_eq!(
            response.body.body.data.data["transaction_hash"],
            tx_hash.unwrap()
        );
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_key_creation_order_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.order_status_table_name;

    let new_order_id = Uuid::new_v4();

    let order = put_key_creation_order(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        new_order_id,
        OrderState::Completed,
    )
    .await;

    let input = build_input(new_order_id);
    let response: OrderResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}. {e:?}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(order.order_id, response.body.body.order_id);
    assert_eq!(order.order_type, response.body.body.order_type);
    assert_eq!(order.order_version, response.body.body.order_version);
    assert_eq!(order.created_at, response.body.body.created_at);
    assert_eq!(order.last_modified_at, response.body.body.last_modified_at);
    assert_eq!(order.state, response.body.body.state);
    assert_eq!(order.data, response.body.body.data);
}

#[rstest]
#[tokio::test]
pub async fn mpc_fetch_order_id_not_found(fixture: &LambdaFixture) {
    let invalid_order_id = "00000000-e29b-41d4-a716-446655440000";
    let input = build_input(Uuid::from_str(invalid_order_id).unwrap());
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::NOT_FOUND, response.body.status_code);
    assert_eq!(ORDER_NOT_FOUND, response.body.body.code);
    assert!(response.body.body.message.contains(invalid_order_id));
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_fetch_speedup_returns_404(fixture: &LambdaFixture, repo_fixture: &RepoFixture) {
    let order_id = Uuid::new_v4();
    let order_status = OrderStatus {
        order_id,
        order_version: "1".to_owned(),
        state: OrderState::Submitted,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            },
            data: speedup_data(),
        },
        created_at: Utc::now(),
        order_type: OrderType::SpeedUp,
        last_modified_at: Utc::now(),
        replaced_by: None,
        replaces: Some(order_id),
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    repo_fixture
        .orders_repository
        .create_order(&order_status)
        .await
        .expect("item not inserted");

    let input = build_input(order_id);
    let response: ErrorResponse = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::NOT_FOUND, response.body.status_code);
    assert_eq!(ORDER_NOT_FOUND, response.body.body.code);
    assert!(response.body.body.message.contains(&order_id.to_string()));
}
