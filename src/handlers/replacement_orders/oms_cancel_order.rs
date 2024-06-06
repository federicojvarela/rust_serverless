use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::step_functions::get_step_functions_client;
use common::serializers::h160::h160_to_lowercase_hex_string;
use config::Config;
use dtos::ReplacementRequestType;
use ethers::types::{Bytes, H160, U256};
use hex::FromHex;
use http::{Response, StatusCode};
use lambda_http::{run, Error, Request};
use model::order::{OrderState, OrderStatus, OrderTransaction, OrderType, SignatureOrderData};
use mpc_signature_sm::authorization::{
    AuthorizationProviderByOrder, AuthorizationProviderByOrderImpl,
};
use mpc_signature_sm::dtos::requests::send_to_approvers_sm::SendToApproversStateMachineRequest;
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::orders_repository_error::{
    orders_repository_error_into_http_response, ORDER_NOT_FOUND,
};
use mpc_signature_sm::http::errors::{
    not_found_response, unknown_error_response, validation_error_response,
    INCOMPATIBLE_ORDER_REPLACEMENT_ERROR_MESSAGE,
};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::{
    invoke_step_function_async, StepFunctionConfig,
};
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::model::step_function::StepFunctionContext;
use mpc_signature_sm::result::error::LambdaError;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::{OrdersRepository, OrdersRepositoryError};
use rusoto_stepfunctions::StepFunctions;
use tower::service_fn;
use uuid::Uuid;
use validations::validate_order_type;

use crate::data::build_replacement_order;
use crate::dtos::ReplacementRequest;
use crate::validations::validate_new_gas_values;

mod config;
mod data;
mod dtos;
mod validations;

pub const ORDER_ID: &str = "order_id";

pub struct State<SF: StepFunctions, OR: OrdersRepository, A: AuthorizationProviderByOrder> {
    pub config: Config,
    pub authorization_provider: A,
    pub orders_repository: Arc<OR>,
    pub step_functions_client: SF,
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
            authorization_provider,
            orders_repository,
            step_functions_client: get_step_functions_client(),
        }
    },
    oms_cancel_order
);

async fn oms_cancel_order(
    request: Request,
    state: &State<impl StepFunctions, impl OrdersRepository, impl AuthorizationProviderByOrder>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let original_order_id: Uuid = request.extract_path_param(ORDER_ID)?;
    let client_id: String = request.extract_client_id()?;

    let has_permission = state
        .authorization_provider
        .client_id_has_order_permission(original_order_id, &client_id)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    if !has_permission {
        return Err(not_found_response(
            ORDER_NOT_FOUND,
            original_order_id.to_string(),
        ));
    }

    tracing::info!(
        order_id = ?original_order_id,
        "cancelling order id {}",
        original_order_id
    );

    let mark_for_cancellation = state
        .orders_repository
        .request_cancellation(original_order_id.to_string())
        .await;

    match mark_for_cancellation {
        // If we fall in this case, means the order has not been SUBMITTED or further, meaning that
        // we can cancel it without sending a replacement transaction to the chain.
        // The state check is done in the query though a ConditionalCheck to avoid race conditions
        // (that's why the next match conditoin is evaluating a ConditionalCheckFailed error)
        Ok(_) => (),

        // If we fall in this case, means that the order is SUBMITTED, meaning that we need to send
        // a replacement transaction to the chain.
        Err(OrdersRepositoryError::ConditionalCheckFailed(_)) => {
            let original_order = state
                .orders_repository
                .get_order_by_id(original_order_id.to_string())
                .await
                .map_err(orders_repository_error_into_http_response)?;

            validate_order(&original_order)?;

            let signature_order_data = original_order.extract_signature_data().map_err(|e| {
                unknown_error_response(LambdaError::Unknown(anyhow!(
            "there was an error extracting the signature data from the original order.{e:?}"
        )))
            })?;

            let body: ReplacementRequest = request.extract_body()?;

            let (from_address, _) = match original_order.data.extract_address_and_chain_id() {
                Ok((address, chain_id)) => (address, chain_id),
                Err(e) => {
                    return Err(unknown_error_response(LambdaError::Unknown(
                        anyhow!(e).context("Failed to find address or chain_id for the order"),
                    )))
                }
            };

            validate_new_gas_values(&signature_order_data.data.transaction, &body)?;

            let new_signature_order_data =
                build_signature_order_data(&signature_order_data.data, &body, from_address)?;

            let cancellation_order = build_replacement_order(
                &original_order,
                &new_signature_order_data,
                OrderType::Cancellation,
            )?;

            tracing::info!(
                order_id = ?original_order_id,
                "Cancelling order id {}",
                original_order_id.to_string()
            );

            state
                .orders_repository
                .create_order(&cancellation_order)
                .await
                .map_err(|e| {
                    unknown_error_response(LambdaError::Unknown(
                        anyhow!(e).context("Error creating cancellation order"),
                    ))
                })?;

            let steps_config = StepFunctionConfig::from(&state.config);
            let steps_function_request =
                serde_json::to_value(&SendToApproversStateMachineRequest {
                    context: StepFunctionContext {
                        order_id: cancellation_order.order_id,
                    },
                    payload: new_signature_order_data,
                })
                .map_err(|e| {
                    unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                        "unable to create state machine json payload: {e:?}"
                    )))
                })?;

            invoke_step_function_async(
                client_id,
                steps_function_request,
                &state.step_functions_client,
                &steps_config,
                cancellation_order.order_id.to_string(),
            )
            .await
            .map_err(unknown_error_response)?;
        }
        Err(order_repository_error) => {
            return Err(orders_repository_error_into_http_response(
                order_repository_error,
            ))
        }
    }

    LambdaProxyHttpResponse {
        status_code: StatusCode::ACCEPTED,
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

pub fn build_signature_order_data(
    original_signature_order_data: &SignatureOrderData,
    request: &ReplacementRequest,
    from_address: H160,
) -> Result<SignatureOrderData, Response<String>> {
    let mut signature_data = original_signature_order_data.clone();

    let zero_bytes_value = Bytes::from_hex("0x00").map_err(|e| {
        unknown_error_response(LambdaError::Unknown(anyhow!(
            "Failed to create a value with zero bytes.{e:?}"
        )))
    })?;

    match (&mut signature_data.transaction, &request.transaction) {
        (
            OrderTransaction::Legacy {
                ref mut gas_price,
                ref mut data,
                ref mut value,
                ref mut to,
                ..
            },
            ReplacementRequestType::Legacy {
                gas_price: new_gas_price,
            },
        ) => {
            *gas_price = *new_gas_price;
            *data = zero_bytes_value;
            *value = U256::from(0);
            *to = h160_to_lowercase_hex_string(from_address);
            Ok(signature_data)
        }
        (
            OrderTransaction::Eip1559 {
                ref mut max_priority_fee_per_gas,
                ref mut max_fee_per_gas,
                ref mut data,
                ref mut value,
                ref mut to,
                ..
            },
            ReplacementRequestType::Eip1559 {
                max_fee_per_gas: new_max_fee_per_gas,
                max_priority_fee_per_gas: new_max_priority_fee_per_gas,
            },
        ) => {
            *max_priority_fee_per_gas = *new_max_priority_fee_per_gas;
            *max_fee_per_gas = *new_max_fee_per_gas;
            *data = zero_bytes_value;
            *value = U256::from(0);
            *to = h160_to_lowercase_hex_string(from_address);
            Ok(signature_data)
        }
        _ => Err(validation_error_response(
            INCOMPATIBLE_ORDER_REPLACEMENT_ERROR_MESSAGE.to_string(),
            None,
        ))?,
    }
}

pub fn validate_order(original_order: &OrderStatus) -> Result<(), Response<String>> {
    validate_order_type(original_order)?;
    validate_order_state(original_order)?;

    Ok(())
}

fn validate_order_state(original_order: &OrderStatus) -> Result<(), Response<String>> {
    tracing::info!(order_id = ?original_order.order_id, order_state = ?original_order.state, "validating order for cancellation");
    if original_order.state != OrderState::Submitted {
        return Err(validation_error_response(
            "can't perform this operation because the order has reached a terminal state"
                .to_string(),
            None,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;

    use async_trait::async_trait;
    use ethers::types::{Bytes, H160, U256};
    use hex::FromHex;
    use lambda_http::{Body, RequestExt};
    use mockall::mock;
    use mockall::predicate::eq;
    use rstest::{fixture, rstest};
    use rusoto_stepfunctions::StartExecutionOutput;
    use serde::Deserialize;
    use serde_json::json;
    use uuid::Uuid;

    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, GAS_PRICE_PLUS_ONE_FOR_MOCK_REQUESTS,
        MAX_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS,
        MAX_PRIORITY_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS, VALUE_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::http::helpers::build_request_custom_auth;
    use common::test_tools::mocks::step_client::MockStepsClient;
    use model::order::helpers::{
        build_signature_order, signature_order_eip1559_data, signature_order_legacy_data,
    };
    use model::order::{OrderState, OrderTransaction, SignatureOrderData};
    use mpc_signature_sm::authorization::AuthorizationProviderError;
    use mpc_signature_sm::http::errors::orders_repository_error::ORDER_NOT_FOUND;
    use mpc_signature_sm::http::errors::VALIDATION_ERROR_CODE;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepositoryError;

    use super::*;
    use crate::validations::tests::{
        build_eip1559_request, build_legacy_request, check_incompatible_order_replacement_error,
    };

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
        pub step_function_client: MockStepsClient,
    }

    #[derive(Debug, Deserialize)]
    pub struct OrderAcceptedBody {
        pub order_id: Uuid,
    }

    #[derive(Deserialize, Debug)]
    pub struct LambdaErrorResponse {
        pub code: String,
        pub message: String,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = ConfigLoader::load_test::<Config>();
        TestFixture {
            config,
            authorization_provider: MockAuthProvider::new(),
            orders_repository: MockOrdersRepository::new(),
            step_function_client: MockStepsClient::new(),
        }
    }

    #[tokio::test]
    async fn build_signature_order_eip1559_data_ok() {
        let original_order_data = signature_order_eip1559_data(VALUE_FOR_MOCK_REQUESTS.into());
        let replacement_request = build_eip1559_request(
            U256::from(MAX_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS),
            U256::from(MAX_PRIORITY_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS),
        );
        let from_address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();

        let response: SignatureOrderData =
            build_signature_order_data(&original_order_data, &replacement_request, from_address)
                .unwrap();
        match response.transaction {
            OrderTransaction::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                data,
                value,
                to,
                ..
            } => {
                assert_eq!(
                    max_fee_per_gas,
                    MAX_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS.into()
                );
                assert_eq!(
                    max_priority_fee_per_gas,
                    MAX_PRIORITY_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS.into()
                );
                assert_eq!(Bytes::from_hex("0x00").unwrap(), data);
                assert_eq!(U256::from(0), value);
                assert_eq!(ADDRESS_FOR_MOCK_REQUESTS, to);
            }
            _ => panic!("invalid transaction"),
        }
    }

    #[tokio::test]
    async fn build_signature_order_legacy_data_ok() {
        let original_order_data = signature_order_legacy_data(VALUE_FOR_MOCK_REQUESTS.into());
        let replacement_request =
            build_legacy_request(U256::from(GAS_PRICE_PLUS_ONE_FOR_MOCK_REQUESTS));
        let from_address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();

        let response: SignatureOrderData =
            build_signature_order_data(&original_order_data, &replacement_request, from_address)
                .unwrap();
        match response.transaction {
            OrderTransaction::Legacy {
                gas_price,
                data,
                value,
                to,
                ..
            } => {
                assert_eq!(gas_price, GAS_PRICE_PLUS_ONE_FOR_MOCK_REQUESTS.into());
                assert_eq!(Bytes::from_hex("0x00").unwrap(), data);
                assert_eq!(U256::from(0), value);
                assert_eq!(ADDRESS_FOR_MOCK_REQUESTS, to);
            }
            _ => panic!("invalid transaction"),
        }
    }

    #[tokio::test]
    async fn eip_order_legacy_request_fail() {
        let original_eip_order_data = signature_order_eip1559_data(VALUE_FOR_MOCK_REQUESTS.into());
        let legacy_replacement_request =
            build_legacy_request(U256::from(GAS_PRICE_PLUS_ONE_FOR_MOCK_REQUESTS));
        let from_address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();

        let response = build_signature_order_data(
            &original_eip_order_data,
            &legacy_replacement_request,
            from_address,
        )
        .unwrap_err();

        check_incompatible_order_replacement_error(response);
    }

    #[tokio::test]
    async fn legacy_order_eip_request_fail() {
        let original_legacy_order_data =
            signature_order_legacy_data(VALUE_FOR_MOCK_REQUESTS.into());
        let eip_replacement_request = build_eip1559_request(
            U256::from(MAX_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS),
            U256::from(MAX_PRIORITY_FEE_PER_GAS_PLUS_ONE_FOR_MOCK_REQUESTS),
        );
        let from_address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();

        let response = build_signature_order_data(
            &original_legacy_order_data,
            &eip_replacement_request,
            from_address,
        )
        .unwrap_err();

        check_incompatible_order_replacement_error(response);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_response_is_valid_ok(mut fixture: TestFixture) {
        let client_id = "some_client_id";
        let auth = json!({ "client_id": client_id });
        let order_id = Uuid::new_v4();
        let path_params = HashMap::from([("order_id".to_owned(), order_id.to_string())]);
        let request =
            build_request_custom_auth(auth, Body::default()).with_path_parameters(path_params);

        fixture
            .orders_repository
            .expect_request_cancellation()
            .with(eq(order_id.to_string()))
            .once()
            .returning(|_| Ok(()));

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(client_id))
            .returning(|_, _| Ok(true));

        let response = oms_cancel_order(
            request,
            &State {
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
                orders_repository: Arc::new(fixture.orders_repository),
                step_functions_client: fixture.step_function_client,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::ACCEPTED, response.status());
    }

    #[rstest]
    #[tokio::test]
    async fn handle_response_order_not_found(mut fixture: TestFixture) {
        let client_id = "some_client_id";
        let auth = json!({ "client_id": client_id });
        let order_id = Uuid::new_v4();
        let path_params = HashMap::from([("order_id".to_owned(), order_id.to_string())]);
        let request =
            build_request_custom_auth(auth, Body::default()).with_path_parameters(path_params);

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(client_id))
            .returning(|_, _| Ok(false));

        let response = oms_cancel_order(
            request,
            &State {
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
                orders_repository: Arc::new(fixture.orders_repository),
                step_functions_client: fixture.step_function_client,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!(ORDER_NOT_FOUND, body.code);
        assert_eq!(order_id.to_string(), body.message);
    }

    #[rstest]
    #[case::cancelled(OrderState::Cancelled)]
    #[case::completed(OrderState::Completed)]
    #[case::completed_with_error(OrderState::CompletedWithError)]
    #[case::dropped(OrderState::Dropped)]
    #[case::error(OrderState::Error)]
    #[case::not_signed(OrderState::NotSigned)]
    #[case::not_submitted(OrderState::NotSubmitted)]
    #[case::reorged(OrderState::Reorged)]
    #[case::replaced(OrderState::Replaced)]
    #[tokio::test]
    async fn handle_response_order_not_cancellable_state(
        mut fixture: TestFixture,
        #[case] original_order_state: OrderState,
    ) {
        let client_id = "some_client_id";
        let auth = json!({ "client_id": client_id });
        let original_order_id = Uuid::new_v4();
        let error_msg = format!(
            "can't perform this operation for an order in {} state",
            original_order_state
        );
        let order_id = Uuid::new_v4();
        let path_params = HashMap::from([("order_id".to_owned(), order_id.to_string())]);
        let request =
            build_request_custom_auth(auth, Body::default()).with_path_parameters(path_params);

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(client_id))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_request_cancellation()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| {
                Err(OrdersRepositoryError::ConditionalCheckFailed(
                    error_msg.to_owned(),
                ))
            });

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| {
                Ok(build_signature_order(
                    original_order_id,
                    original_order_state,
                    None,
                ))
            });

        let response = oms_cancel_order(
            request,
            &State {
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
                orders_repository: Arc::new(fixture.orders_repository),
                step_functions_client: fixture.step_function_client,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!(VALIDATION_ERROR_CODE, body.code);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_cancel_order_send_to_chain(mut fixture: TestFixture) {
        let client_id = "some_client_id";
        let auth = json!({ "client_id": client_id });
        let error_msg = "internal server error";
        let original_order_id = Uuid::new_v4();
        let original_order = build_signature_order(original_order_id, OrderState::Submitted, None);
        let order_id = Uuid::new_v4();
        let path_params = HashMap::from([("order_id".to_owned(), order_id.to_string())]);
        let cancellation_request =
            build_eip1559_request(U256::from("0x12a05f201"), U256::from("0x12a15f201"));
        let body = Body::Text(json!(cancellation_request).to_string());
        let request = build_request_custom_auth(auth, body).with_path_parameters(path_params);

        fixture
            .orders_repository
            .expect_request_cancellation()
            .with(eq(order_id.to_string()))
            .once()
            .returning(|_| {
                Err(OrdersRepositoryError::ConditionalCheckFailed(
                    error_msg.to_string(),
                ))
            });

        fixture
            .authorization_provider
            .expect_client_id_has_order_permission()
            .once()
            .with(eq(order_id), eq(client_id))
            .returning(|_, _| Ok(true));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(original_order.clone()));

        fixture
            .orders_repository
            .expect_create_order()
            .once()
            .returning(move |_| Ok(()));

        fixture
            .step_function_client
            .expect_start_execution()
            .once()
            .returning(|_| Ok(StartExecutionOutput::default()));

        let response = oms_cancel_order(
            request,
            &State {
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
                orders_repository: Arc::new(fixture.orders_repository),
                step_functions_client: fixture.step_function_client,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::ACCEPTED, response.status());
    }

    #[rstest]
    #[case::completed(OrderState::Completed)]
    #[case::received(OrderState::Received)]
    #[case::compliance(OrderState::ApproversReviewed)]
    #[case::signed(OrderState::Signed)]
    #[case::cancelled(OrderState::Cancelled)]
    #[case::error(OrderState::Error)]
    #[tokio::test]
    async fn order_not_valid_state_error(#[case] order_state: OrderState) {
        use ethers::types::H256;
        use serde_json::Value;

        let order = build_signature_order(
            Uuid::new_v4(),
            order_state,
            Some(H256::random().to_string()),
        );
        let result = validate_order_state(&order);
        assert!(result.is_err());
        let json: Value =
            serde_json::from_str(result.unwrap_err().body()).expect("JSON parsing failed");
        assert_eq!(json["code"], "validation");
        assert_eq!(
            json["message"],
            "can't perform this operation because the order has reached a terminal state",
        );
    }
}
