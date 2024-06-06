use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use lambda_http::{run, service_fn, Error, Request};
use reqwest::StatusCode;
use uuid::Uuid;

use common::aws_clients::dynamodb::get_dynamodb_client;
use config::Config;
use model::order::OrderType;
use model::order::{OrderState, OrderStatus};
use models::OrderResponse;
use mpc_signature_sm::authorization::{
    AuthorizationProviderByOrder, AuthorizationProviderByOrderImpl,
};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::orders_repository_error::orders_repository_error_into_http_response;
use mpc_signature_sm::http::errors::{not_found_response, unknown_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::result::error::LambdaError;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

mod config;
mod models;

pub const ORDER_ID_PATH_PARAM: &str = "order_id";
pub const ORDER_NOT_FOUND: &str = "order_not_found";

const PENDING_STATE: &[OrderState; 3] = &[
    OrderState::Received,
    OrderState::Signed,
    OrderState::ApproversReviewed,
];

const MINTED_STATE: &[OrderState; 2] = &[OrderState::Completed, OrderState::CompletedWithError];
const ERROR_FINAL_STATE: &[OrderState; 2] = &[OrderState::NotSubmitted, OrderState::Error];

pub struct State<OR: OrdersRepository, A: AuthorizationProviderByOrder> {
    pub config: Config,
    pub orders_repository: Arc<OR>,
    pub authorization_provider: A,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            get_dynamodb_client(),
        ));
        let authorization_provider =
            AuthorizationProviderByOrderImpl::new(orders_repository.clone());

        State {
            config,
            orders_repository,
            authorization_provider,
        }
    },
    mpc_fetch_order
);

async fn mpc_fetch_order(
    request: Request,
    state: &State<impl OrdersRepository, impl AuthorizationProviderByOrder>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let app_client_id = request.extract_client_id()?;
    let order_id: Uuid = request.extract_path_param(ORDER_ID_PATH_PARAM)?;

    let has_permission = state
        .authorization_provider
        .client_id_has_order_permission(order_id, &app_client_id)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;
    if !has_permission {
        return Err(not_found_response(ORDER_NOT_FOUND, order_id.to_string()));
    }

    let order = state
        .orders_repository
        .get_order_by_id(order_id.to_string())
        .await
        .map_err(orders_repository_error_into_http_response)?;

    let response = match order.order_type {
        OrderType::KeyCreation | OrderType::Sponsored => order,

        // We don't return speedup orders to the user.
        OrderType::SpeedUp | OrderType::Cancellation => {
            return Err(not_found_response(
                ORDER_NOT_FOUND,
                format!("order_id {order_id} not found"),
            ));
        }

        OrderType::Signature => {
            // Here the signature order was replaced by another order. We probably need to merge
            // data from the replacing order into this one before returning the order to the user.
            if let Some(replacing_order_id) = order.replaced_by {
                let replacing_order = state
                    .orders_repository
                    .get_order_by_id(replacing_order_id.to_string())
                    .await
                    .map_err(orders_repository_error_into_http_response)?;

                // Here are three cases:
                //
                // 1. If the original order was minted, we return it. It does not matter if there
                //    is a replacement order, it will fail because it contains the same nonce as
                //    this one.
                // 2. The replacement order was not submitted yet, so it is not in mempool. In
                //    this case we also show the original order until the replacement gets to the
                //    mempool.
                // 3. The replacement order is not submitted or has an error.
                //
                // In both cases was last modified when the replacement order was modified though.
                if MINTED_STATE.contains(&order.state)
                    || PENDING_STATE.contains(&replacing_order.state)
                    || ERROR_FINAL_STATE.contains(&replacing_order.state)
                {
                    OrderStatus {
                        last_modified_at: replacing_order.last_modified_at,
                        ..order
                    }
                }
                // If we are not in the above cases we need to keep the original order ID, but
                // replace some data, such as the state, the data fields (that contain new maestro
                // signature, new tx data, etc) and the last_modified_at
                else {
                    // If the replacement order is a Cancellation order, and it is complete, we
                    // show to the user the Cancelled state
                    let state = if replacing_order.order_type == OrderType::Cancellation
                        && replacing_order.state == OrderState::Completed
                    {
                        OrderState::Cancelled
                    } else {
                        replacing_order.state
                    };

                    OrderStatus {
                        state,
                        data: replacing_order.data,
                        last_modified_at: replacing_order.last_modified_at,
                        transaction_hash: replacing_order.transaction_hash,
                        ..order
                    }
                }
            } else {
                order
            }
        }
    };

    let body = serde_json::to_string(&OrderResponse::from(response))
        .map_err(|e| unknown_error_response(LambdaError::Unknown(anyhow!("{e}"))))?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        body: Some(body),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

#[cfg(test)]
mod tests {
    use std::assert_eq;
    use std::collections::HashMap;
    use std::str::FromStr;

    use async_trait::async_trait;
    use chrono::{Days, Utc};
    use lambda_http::{Body, Request, RequestExt};
    use mockall::mock;
    use mockall::predicate::eq;
    use reqwest::StatusCode;
    use rstest::*;
    use serde_json::json;

    use common::test_tools::http::constants::{
        CLIENT_ID_FOR_MOCK_REQUESTS, HASH_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
        TX_HASH_ERROR_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::http::helpers::build_request_custom_auth;
    use model::order::helpers::{
        build_cancellation_order, build_signature_order, build_speedup_order, build_sponsored_order,
    };
    use mpc_signature_sm::authorization::AuthorizationProviderError;
    use mpc_signature_sm::http::errors::{validation_error_response, SERVER_ERROR_CODE};
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepositoryError;

    use super::*;

    mock! {
        AuthProvider {}
        #[async_trait]
        impl AuthorizationProviderByOrder for AuthProvider {
            async fn client_id_has_order_permission(
                &self,
                order_id: Uuid,
                client_id: &str,
            ) -> Result<bool, AuthorizationProviderError>;
        }
    }

    struct TestFixture {
        pub config: Config,
        pub orders_repository: MockOrdersRepository,
        pub authorization_provider: MockAuthProvider,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = ConfigLoader::load_test::<Config>();

        TestFixture {
            config,
            orders_repository: MockOrdersRepository::new(),
            authorization_provider: MockAuthProvider::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_missing_order_id_param(mut fixture: TestFixture) {
        fixture.orders_repository.expect_get_order_by_id().never();
        assert_expected_body_and_status(
            fixture,
            "",
            StatusCode::BAD_REQUEST,
            validation_error_response("order_id not found in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_order_id_not_valid_uuidv4(mut fixture: TestFixture) {
        let order_id = "NOT_A_VALID_UUID_FORMAT";
        fixture.orders_repository.expect_get_order_by_id().never();
        assert_expected_body_and_status(
            fixture,
            order_id,
            StatusCode::BAD_REQUEST,
            validation_error_response("order_id with wrong type in request path".to_owned(), None)
                .body(),
        )
        .await;
    }

    #[rstest]
    #[tokio::test]
    async fn handle_incorrect_client_id(mut fixture: TestFixture) {
        let order_id = ORDER_ID_FOR_MOCK_REQUESTS;
        let order_uuid = Uuid::from_str(order_id).unwrap();
        let client_id = "wrong-client-id";

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_uuid), eq(client_id))
            .returning(|_, _| Ok(false));

        fixture.orders_repository.expect_get_order_by_id().never();

        let request = request_with_order_id(order_id.to_string(), client_id);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await;
        let response = response.unwrap_or_else(|response| response);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let order_body = response.body().as_str();
        assert!(order_body.contains(order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn handle_existing_order_ok(mut fixture: TestFixture) {
        let order_id = ORDER_ID_FOR_MOCK_REQUESTS;
        let order_uuid = Uuid::from_str(order_id).unwrap();
        let client_id = CLIENT_ID_FOR_MOCK_REQUESTS;
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_uuid), eq(client_id))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_owned()))
            .once()
            .returning(move |_| {
                Ok(build_signature_order(
                    order_uuid,
                    OrderState::Completed,
                    None,
                ))
            });

        assert_expected_body_and_status(fixture, order_id, StatusCode::OK, order_id).await;
    }

    #[rstest]
    #[case::replacement_in_received_state(OrderState::Received)]
    #[case::replacement_in_signed_state(OrderState::Signed)]
    #[case::replacement_in_approver_reviewed_state(OrderState::ApproversReviewed)]
    #[case::replacement_in_not_submitted_state(OrderState::NotSubmitted)]
    #[case::replacement_in_error_state(OrderState::Error)]
    #[tokio::test]
    async fn handle_return_original_order_replacement_in_pre_submit_state(
        mut fixture: TestFixture,
        #[case] replacement_state: OrderState,
    ) {
        let order_id = Uuid::new_v4();
        let replacement_order_id = Uuid::new_v4();
        let mut order_status = build_signature_order(order_id, OrderState::Submitted, None);
        order_status.replaced_by = Some(replacement_order_id);

        let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
        let replacement_order = build_speedup_order(
            replacement_order_id,
            replacement_state,
            order_id,
            replacement_last_modified_at,
        );

        // Original order
        let db_order_status = order_status.clone();
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_order_status.clone()));

        // Replacement order
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(replacement_order_id.to_string()))
            .once()
            .returning(move |_| Ok(replacement_order.clone()));

        let expected_order = OrderResponse::from(OrderStatus {
            last_modified_at: replacement_last_modified_at,
            ..order_status
        });

        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let order_body = serde_json::from_str::<OrderResponse>(response.body().as_str()).unwrap();
        assert_eq!(expected_order, order_body);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_return_cancellation_with_original_order_id_and_correct_state(
        mut fixture: TestFixture,
    ) {
        let order_id = Uuid::new_v4();
        let replacement_order_id = Uuid::new_v4();
        let mut order_status = build_signature_order(
            order_id,
            OrderState::Submitted,
            Some(HASH_FOR_MOCK_REQUESTS.to_string()),
        );
        order_status.replaced_by = Some(replacement_order_id);

        let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
        let mut replacement_order = build_cancellation_order(
            replacement_order_id,
            OrderState::Completed,
            order_id,
            Some(replacement_last_modified_at),
            None,
        );
        replacement_order.transaction_hash = Some(TX_HASH_ERROR_FOR_MOCK_REQUESTS.to_string());

        // Original order
        let db_order_status = order_status.clone();
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_order_status.clone()));

        // Replacement order
        let db_replacement_order_status = replacement_order.clone();
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(replacement_order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_replacement_order_status.clone()));

        let expected_order = OrderResponse::from(OrderStatus {
            transaction_hash: replacement_order.transaction_hash,
            state: OrderState::Cancelled,
            data: replacement_order.data,
            last_modified_at: replacement_last_modified_at,
            ..order_status
        });

        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let order_body = serde_json::from_str::<OrderResponse>(response.body().as_str()).unwrap();
        assert_eq!(expected_order, order_body);
    }

    #[rstest]
    #[case::replacement_in_submitted_state(OrderState::Submitted)]
    #[case::replacement_in_completed_state(OrderState::Completed)]
    #[case::replacement_in_completed_with_error_state(OrderState::CompletedWithError)]
    #[tokio::test]
    async fn handle_return_replacement_with_original_order_id(
        mut fixture: TestFixture,
        #[case] replacement_state: OrderState,
    ) {
        let order_id = Uuid::new_v4();
        let replacement_order_id = Uuid::new_v4();
        let mut order_status = build_signature_order(
            order_id,
            OrderState::Submitted,
            Some(HASH_FOR_MOCK_REQUESTS.to_string()),
        );
        order_status.replaced_by = Some(replacement_order_id);

        let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
        let mut replacement_order = build_speedup_order(
            replacement_order_id,
            replacement_state,
            order_id,
            replacement_last_modified_at,
        );
        replacement_order.transaction_hash = Some(TX_HASH_ERROR_FOR_MOCK_REQUESTS.to_string());

        // Original order
        let db_order_status = order_status.clone();
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_order_status.clone()));

        // Replacement order
        let db_replacement_order_status = replacement_order.clone();
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(replacement_order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_replacement_order_status.clone()));

        let expected_order = OrderResponse::from(OrderStatus {
            transaction_hash: replacement_order.transaction_hash,
            state: replacement_order.state,
            data: replacement_order.data,
            last_modified_at: replacement_last_modified_at,
            ..order_status
        });

        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let order_body = serde_json::from_str::<OrderResponse>(response.body().as_str()).unwrap();
        assert_eq!(expected_order, order_body);
    }

    #[rstest]
    #[case::original_in_completed_state(OrderState::Completed)]
    #[case::original_in_completed_with_error_state(OrderState::CompletedWithError)]
    #[tokio::test]
    async fn handle_return_original_order_if_minted_ok(
        mut fixture: TestFixture,
        #[case] original_order_state: OrderState,
    ) {
        let order_id = Uuid::new_v4();
        let replacement_order_id = Uuid::new_v4();
        let mut order_status = build_signature_order(order_id, original_order_state, None);
        order_status.replaced_by = Some(replacement_order_id);

        let replacement_last_modified_at = Utc::now().checked_add_days(Days::new(1)).unwrap();
        let replacement_order = build_speedup_order(
            replacement_order_id,
            OrderState::Received,
            order_id,
            replacement_last_modified_at,
        );

        // Original order
        let db_order_status = order_status.clone();
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_order_status.clone()));

        // Replacement order
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(replacement_order_id.to_string()))
            .once()
            .returning(move |_| Ok(replacement_order.clone()));

        let expected_order = OrderResponse::from(OrderStatus {
            last_modified_at: replacement_last_modified_at,
            ..order_status
        });

        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let order_body = serde_json::from_str::<OrderResponse>(response.body().as_str()).unwrap();
        assert_eq!(expected_order, order_body);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_speedup_order_not_found(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4(); // just some random order_id that doesn't exist in db;
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| {
                Ok(build_speedup_order(
                    order_id,
                    OrderState::Completed,
                    Uuid::new_v4(),
                    Utc::now(),
                ))
            });

        assert_expected_body_and_status(
            fixture,
            &order_id.to_string(),
            StatusCode::NOT_FOUND,
            ORDER_NOT_FOUND,
        )
        .await;
    }

    #[rstest]
    #[tokio::test]
    async fn handle_return_sponsored_order(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let replacement_order_id = Uuid::new_v4();
        let mut order_status = build_sponsored_order(order_id, OrderState::Signed);
        order_status.replaced_by = Some(replacement_order_id);

        // Original order
        let db_order_status = order_status.clone();
        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(db_order_status.clone()));

        let expected_order = OrderResponse::from(order_status);

        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let order_body = serde_json::from_str::<OrderResponse>(response.body().as_str()).unwrap();
        assert_eq!(expected_order, order_body);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_order_not_found(mut fixture: TestFixture) {
        let order_id = "c94ec32d-61e4-47ad-9d9d-50d3e2d0c807"; // just some random order_id that doesn't exist in db;
        let order_uuid = Uuid::from_str(order_id).unwrap();

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_uuid), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_owned()))
            .once()
            .returning(move |_| Err(OrdersRepositoryError::OrderNotFound("not found".to_owned())));

        assert_expected_body_and_status(fixture, order_id, StatusCode::NOT_FOUND, ORDER_NOT_FOUND)
            .await;
    }

    #[rstest]
    #[tokio::test]
    async fn handle_db_error(mut fixture: TestFixture) {
        let order_id = ORDER_ID_FOR_MOCK_REQUESTS;
        let order_uuid = Uuid::from_str(order_id).unwrap();

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_uuid), eq(CLIENT_ID_FOR_MOCK_REQUESTS))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_owned()))
            .once()
            .returning(move |_| Err(OrdersRepositoryError::Unknown(anyhow!("timeout!"))));

        assert_expected_body_and_status(
            fixture,
            order_id,
            StatusCode::INTERNAL_SERVER_ERROR,
            SERVER_ERROR_CODE,
        )
        .await;
    }

    fn request_with_order_id(order_id: String, client_id: &str) -> Request {
        let auth = json!({ "client_id": client_id });
        let path_params: HashMap<String, String> = if !order_id.is_empty() {
            let mut hm = HashMap::new();
            hm.insert(ORDER_ID_PATH_PARAM.to_string(), order_id);
            hm
        } else {
            HashMap::new()
        };
        build_request_custom_auth(auth, Body::default()).with_path_parameters(path_params)
    }

    async fn assert_expected_body_and_status(
        fixture: TestFixture,
        order_id: &str,
        expected_status: StatusCode,
        expected_body_substring: &str,
    ) {
        let request = request_with_order_id(order_id.to_string(), CLIENT_ID_FOR_MOCK_REQUESTS);
        let response = mpc_fetch_order(
            request,
            &State {
                config: fixture.config.clone(),
                orders_repository: Arc::new(fixture.orders_repository),
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await;

        let response = response.unwrap_or_else(|response| response);

        assert_eq!(response.status(), expected_status);
        let order_body = response.body().as_str();
        assert!(order_body.contains(expected_body_substring));
    }
}
