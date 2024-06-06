use anyhow::anyhow;
use chrono::Utc;
use ethers::types::H160;
use http::{Response, StatusCode};
use lambda_http::{run, Error, Request};
use model::order::{
    GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData, SignatureOrderData,
};
use mpc_signature_sm::dtos::requests::send_to_approvers_sm::SendToApproversStateMachineRequest;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;
use rusoto_stepfunctions::StepFunctions;
use serde_json::json;
use std::sync::Arc;
use tower::service_fn;
use uuid::Uuid;

use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::step_functions::get_step_functions_client;
use config::Config;
use dtos::requests::SignatureRequestBody;
use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
use mpc_signature_sm::authorization::AuthorizationProviderByAddressImpl;
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{not_found_response, unknown_error_response};
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
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::keys::{KeysRepository, KeysRepositoryError};

mod config;
mod dtos;

pub const ADDRESS_INDEX_NAME: &str = "AddressIndex";
pub const ADDRESS: &str = "address";

// TODO: we should have a central place for error codes.
pub const ADDRESS_NOT_FOUND: &str = "address_not_found";

struct State<
    SF: StepFunctions,
    R: KeysRepository,
    OR: OrdersRepository,
    A: AuthorizationProviderByAddress,
> {
    config: Config,
    step_functions_client: SF,
    keys_repository: Arc<R>,
    orders_repository: Arc<OR>,
    authorization_provider: A,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamodb_client.clone(),
        ));

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            dynamodb_client,
        ));

        let authorization_provider =
            AuthorizationProviderByAddressImpl::new(keys_repository.clone());

        State {
            keys_repository,
            orders_repository,
            config,
            authorization_provider,
            step_functions_client: get_step_functions_client(),
        }
    },
    create_signature_order,
    [validate_content_type]
);

async fn create_signature_order(
    request: Request,
    state: &State<
        impl StepFunctions,
        impl KeysRepository,
        impl OrdersRepository,
        impl AuthorizationProviderByAddress,
    >,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    // Validations
    let address: H160 = request.extract_path_param(ADDRESS)?;
    let client_id = request.extract_client_id()?;

    let client_is_allowed = state
        .authorization_provider
        .client_id_has_address_permission(address, &client_id)
        .await
        .map_err(|e| unknown_error_response(e.into()))?;

    if !client_is_allowed {
        return Err(not_found_response(
            ADDRESS_NOT_FOUND,
            "address not found".to_owned(),
        ));
    }

    let body: SignatureRequestBody = request.extract_body()?;
    body.transaction.validate()?;

    let key = state
        .keys_repository
        .get_key_by_address(address)
        .await
        .map_err(|e| match e {
            KeysRepositoryError::Unknown(e) => unknown_error_response(LambdaError::Unknown(e)),
            KeysRepositoryError::KeyNotFound(message) => {
                not_found_response(ADDRESS_NOT_FOUND, message)
            }
        })?;

    // Create the order and save it into the DB
    let order_id = Uuid::new_v4();

    let (new_order, new_order_signature_order_data) =
        create_signature_order_status(order_id, key.key_id, client_id.clone(), address, &body)?;

    state
        .orders_repository
        .create_order(&new_order)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    // Call the approvers SM
    let steps_config = StepFunctionConfig::from(&state.config);
    let steps_function_request = serde_json::to_value(&SendToApproversStateMachineRequest {
        context: StepFunctionContext { order_id },
        payload: new_order_signature_order_data,
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
        new_order.order_id.to_string(),
    )
    .await
    .map_err(unknown_error_response)?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::ACCEPTED,
        body: Some(json!({ "order_id": order_id }).to_string()),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

/// This function creates the order that will be persisted in DynamoDB. It also returns the
/// SignatureOrderData because it is used for calling the approvers state machine.
fn create_signature_order_status(
    id: Uuid,
    key_id: Uuid,
    client_id: String,
    address: H160,
    request_body: &SignatureRequestBody,
) -> Result<(OrderStatus, SignatureOrderData), Response<String>> {
    let signature_order_data = SignatureOrderData {
        transaction: (&request_body.transaction).into(),
        address,
        key_id,
        maestro_signature: None,
    };

    let sign_order_data_json = serde_json::to_value(&signature_order_data).map_err(|e| {
        unknown_error_response(LambdaError::Unknown(
            anyhow!(e).context("Unable to serialize signature order data"),
        ))
    })?;

    let order_status = OrderStatus {
        order_id: id,
        order_version: "1".to_owned(),
        state: OrderState::Received,
        data: GenericOrderData {
            shared_data: SharedOrderData { client_id },
            data: sign_order_data_json,
        },
        order_type: OrderType::Signature,
        created_at: Utc::now(),
        last_modified_at: Utc::now(),
        transaction_hash: None,
        replaced_by: None,
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    Ok((order_status, signature_order_data))
}

#[cfg(test)]
mod tests {
    use ana_tools::config_loader::ConfigLoader;
    use async_trait::async_trait;
    use common::test_tools::dtos::{Error, OrderAcceptedBody};
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::http::helpers::build_request_custom_auth;
    use common::test_tools::mocks::step_client::MockStepsClient;
    use ethers::types::{Address, H160};
    use lambda_http::{Body, Request, RequestExt, Response};
    use mockall::{mock, predicate};
    use model::key::Key;
    use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
    use mpc_signature_sm::authorization::AuthorizationProviderError;
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use repositories::keys::MockKeysRepository;
    use repositories::orders::MockOrdersRepository;
    use reqwest::StatusCode;
    use rstest::{fixture, rstest};
    use rusoto_dynamodb::AttributeValue;
    use rusoto_stepfunctions::*;
    use serde::Serialize;
    use serde_json::json;
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::config::Config;
    use crate::{create_signature_order, State, ADDRESS};

    const VALID_ADDRESS: &str = "0x43400fa4610ebd10d87798e3a90850809d069899";

    #[derive(Serialize)]
    struct KeyDynamoDbResource {
        pub key_id: String,
        pub address: String,
        pub client_id: String,
        pub client_user_id: String,
        pub created_at: String,
        pub order_type: String,
        pub order_version: String,
        pub owning_user_id: String,
        pub public_key: String,
    }

    fn get_key_attributes_map() -> HashMap<String, AttributeValue> {
        serde_dynamo::to_item(KeyDynamoDbResource {
            key_id: KEY_ID_FOR_MOCK_REQUESTS.to_owned(),
            address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            client_user_id: Uuid::default().to_string(),
            created_at: "2023-05-03T16:09:16.997Z".to_owned(),
            order_type: "KEY_CREATION_ORDER".to_owned(),
            order_version: "1".to_owned(),
            owning_user_id: Uuid::default().to_string(),
            public_key: "03762674801475f7a088b26c8cb74d7ccccbd13a7025ed6e38c13b4f261167737c"
                .to_owned(),
        })
        .unwrap()
    }

    mock! {
        AuthProvider {}
        #[async_trait]
        impl AuthorizationProviderByAddress for AuthProvider {
            async fn client_id_has_address_permission(
                &self,
                address: H160,
                client_id: &str,
            ) -> Result<bool, AuthorizationProviderError>;
        }
    }

    struct TestFixture {
        pub config: Config,
        pub step_functions_client: MockStepsClient,
        pub keys_repository: MockKeysRepository,
        pub orders_repository: MockOrdersRepository,
        pub authorization_provider: MockAuthProvider,
    }

    #[fixture]
    fn test_fixture() -> TestFixture {
        TestFixture {
            config: ConfigLoader::load_default::<Config>(),
            step_functions_client: MockStepsClient::new(),
            keys_repository: MockKeysRepository::new(),
            orders_repository: MockOrdersRepository::new(),
            authorization_provider: MockAuthProvider::new(),
        }
    }

    impl TestFixture {
        pub fn get_state(
            self,
        ) -> State<MockStepsClient, MockKeysRepository, MockOrdersRepository, MockAuthProvider>
        {
            State {
                config: self.config,
                step_functions_client: self.step_functions_client,
                keys_repository: Arc::new(self.keys_repository),
                orders_repository: Arc::new(self.orders_repository),
                authorization_provider: self.authorization_provider,
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_response_is_valid_ok(mut test_fixture: TestFixture) {
        test_fixture
            .step_functions_client
            .expect_start_execution()
            .times(1)
            .returning(|_| Ok(StartExecutionOutput::default()));

        test_fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(predicate::eq(
                Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            ))
            .once()
            .returning(move |_| {
                let key: Key = serde_dynamo::from_item(get_key_attributes_map()).unwrap();
                Ok(key)
            });

        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        test_fixture
            .orders_repository
            .expect_create_order()
            .once()
            .returning(move |_| Ok(()));

        let body = Body::Text(
            json!({
                "transaction": {
                    "to": VALID_ADDRESS,
                    "gas": "22000",
                    "gas_price": "300000000",
                    "value": "1",
                    "data": "0x00",
                    "chain_id": 11155111,
                }
            })
            .to_string(),
        );

        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request = build_request_custom_auth(auth, body);
        let request = request.with_path_parameters(HashMap::from([(
            ADDRESS.to_owned(),
            ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        )]));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::ACCEPTED, response.status());
        serde_json::from_str::<OrderAcceptedBody>(response.body().as_str())
            .expect("Could not deserialized success body");
    }

    #[rstest]
    #[tokio::test]
    async fn client_id_not_allowed(mut test_fixture: TestFixture) {
        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(false));

        let body = Body::Text(
            json!({
                "transaction": {
                    "to": VALID_ADDRESS,
                    "gas": "22000",
                    "gas_price": "300000000",
                    "value": "1",
                    "data": "0x00",
                    "chain_id": 11155111,
                }
            })
            .to_string(),
        );
        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request = build_request_custom_auth(auth, body);
        let request = request.with_path_parameters(HashMap::from([(
            ADDRESS.to_owned(),
            ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        )]));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "address_not_found");
        assert_eq!(body.message, "address not found");
    }

    #[rstest]
    #[tokio::test]
    async fn error_missing_address_param(test_fixture: TestFixture) {
        let response: Response<String> = create_signature_order(
            Request::default(),
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, "address not found in request path");
    }

    #[rstest]
    #[tokio::test]
    async fn error_empty_address_param(mut test_fixture: TestFixture) {
        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        let body = Body::Text(
            json!({
                "transaction": {
                    "to": "", // Empty address
                    "gas": "22000",
                    "gas_price": "300000000",
                    "value": "1",
                    "data": "0x00",
                    "chain_id": 11155111,
                }
            })
            .to_string(),
        );

        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request = build_request_custom_auth(auth, body);
        let request = request.with_path_parameters(HashMap::from([(
            ADDRESS.to_owned(),
            ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        )]));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, "to address cannot be empty");
    }

    #[rstest]
    #[tokio::test]
    async fn error_address_not_valid_h160(test_fixture: TestFixture) {
        let request = Request::default()
            .with_path_parameters(HashMap::from([(ADDRESS.to_owned(), "123".to_owned())]));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, "address with wrong type in request path");
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_empty_body(mut test_fixture: TestFixture) {
        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request =
            build_request_custom_auth(auth, Body::default()).with_path_parameters(HashMap::from([
                (ADDRESS.to_owned(), VALID_ADDRESS.to_owned()),
            ]));

        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(VALID_ADDRESS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, "body was empty");
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_invalid_body(mut test_fixture: TestFixture) {
        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request =
            build_request_custom_auth(auth, r#"{ "invalid": json }"#.into()).with_path_parameters(
                HashMap::from([(ADDRESS.to_owned(), VALID_ADDRESS.to_owned())]),
            );

        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(VALID_ADDRESS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(
            body.message,
            "body failed to be converted to a json object".to_owned()
        );
    }

    #[rstest]
    #[tokio::test]
    async fn error_max_priority_fee_bigger_than_max_fee_per_gas(mut test_fixture: TestFixture) {
        test_fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                mockall::predicate::eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                mockall::predicate::eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        let body = Body::Text(
            json!({
                "transaction": {
                    "to": VALID_ADDRESS,
                    "gas": "22000",
                    "max_fee_per_gas": "1500000",
                    "max_priority_fee_per_gas": "1600000",
                    "value": "1",
                    "data": "0x00",
                    "chain_id": 1
                }
            })
            .to_string(),
        );

        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let request = build_request_custom_auth(auth, body);
        let request = request.with_path_parameters(HashMap::from([(
            ADDRESS.to_owned(),
            ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        )]));

        let response: Response<String> = create_signature_order(
            request,
            &test_fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(
            body.message,
            "max_priority_fee_per_gas cannot be bigger than max_fee_per_gas"
        );
    }
}
