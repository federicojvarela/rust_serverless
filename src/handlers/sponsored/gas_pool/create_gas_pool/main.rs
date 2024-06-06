use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::CreateSponsorAddressConfigRequest;
use ethers::types::Address;
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request};
use model::sponsor_address_config::{SponsorAddressConfig, SponsorAddressConfigType};
use mpc_signature_sm::authorization::{
    AuthorizationProviderByAddress, AuthorizationProviderByAddressImpl,
};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{
    not_found_response, unknown_error_response, validation_error_response,
};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::sponsor_address_config::sponsor_address_config_repository_impl::SponsorAddressConfigRepositoryImpl;
use repositories::sponsor_address_config::SponsorAddressConfigRepository;
use std::sync::Arc;
use validator::Validate;

mod config;
mod dtos;

const CHAIN_ID_PATH_PARAM: &str = "chain_id";

const GAS_POOL_LIMIT_PER_CLIENT: usize = 1;
const LIMIT_REACHED_MESSAGE: &str = "Gas pool limit of 1 per client reached.";

const ADDRESS_NOT_FOUND_CODE: &str = "address_not_found";
const ADDRESS_NOT_FOUND_MESSAGE: &str =
    "Address to be set as gas pool was not found, create an address with WaaS to be able to set it as a gas pool.";

pub struct State<T: SponsorAddressConfigRepository, A: AuthorizationProviderByAddress> {
    sponsor_repository: Arc<T>,
    authorization_provider: A,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let sponsor_repository = Arc::new(SponsorAddressConfigRepositoryImpl::new(
            config.sponsor_address_config_table_name.clone(),
            dynamodb_client.clone(),
        ));

        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamodb_client,
        ));

        let authorization_provider =
            AuthorizationProviderByAddressImpl::new(keys_repository.clone());

        State {
            sponsor_repository,
            authorization_provider,
        }
    },
    create_gas_pool,
    [validate_chain_id_is_supported, validate_content_type]
);

async fn create_gas_pool(
    request: Request,
    state: &State<impl SponsorAddressConfigRepository, impl AuthorizationProviderByAddress>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let client_id = request.extract_client_id()?;
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;
    let body = request.extract_body::<CreateSponsorAddressConfigRequest>()?;
    body.validate()
        .map_err(|e| validation_error_response(e.to_string(), None))?;
    let gas_pool_address = body.gas_pool_address;

    let client_is_allowed = state
        .authorization_provider
        .client_id_has_address_permission(gas_pool_address, &client_id)
        .await
        .map_err(|e| unknown_error_response(e.into()))?;

    if !client_is_allowed {
        return Err(not_found_response(
            ADDRESS_NOT_FOUND_CODE,
            ADDRESS_NOT_FOUND_MESSAGE.to_owned(),
        ));
    }

    let result = state
        .sponsor_repository
        .get_addresses(
            client_id.clone(),
            chain_id,
            SponsorAddressConfigType::GasPool,
        )
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    let same_address_exists = result
        .iter()
        .try_fold(false, |a, b| {
            Ok(a || compare_gas_pool(b.clone(), gas_pool_address)?)
        })
        .map_err(unknown_error_response)?;

    if same_address_exists {
        return LambdaProxyHttpResponse {
            status_code: StatusCode::CREATED,
            ..LambdaProxyHttpResponse::default()
        }
        .try_into();
    }

    if result.len() == GAS_POOL_LIMIT_PER_CLIENT {
        return Err(validation_error_response(
            LIMIT_REACHED_MESSAGE.to_owned(),
            None,
        ));
    }

    state
        .sponsor_repository
        .put_address_gas_pool(client_id, chain_id, gas_pool_address)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow!(
                "Error saving gas pool: {e:?}"
            )))
        })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::CREATED,
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

fn compare_gas_pool(
    sponsor_address: SponsorAddressConfig,
    gas_pool_address: Address,
) -> Result<bool, LambdaError> {
    match sponsor_address {
        SponsorAddressConfig::GasPool { address, .. } => Ok(address == gas_pool_address),
        _ => Err(LambdaError::Unknown(anyhow!(
            "Sponsor address config was of wrong type".to_owned()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use common::test_tools::dtos::Error;
    use common::test_tools::http::{
        constants::{
            ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
        },
        helpers::build_request_custom_auth,
    };
    use ethers::types::{Address, H160};
    use http::{Request, StatusCode};
    use lambda_http::{Body, RequestExt};
    use mockall::{mock, predicate::eq};
    use model::sponsor_address_config::SponsorAddressConfig;
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use mpc_signature_sm::{
        authorization::{AuthorizationProviderByAddress, AuthorizationProviderError},
        dtos::responses::http_error::LambdaErrorResponse,
    };
    use repositories::sponsor_address_config::MockSponsorAddressConfigRepository;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use std::{collections::HashMap, str::FromStr, sync::Arc};

    use crate::{
        create_gas_pool, State, ADDRESS_NOT_FOUND_CODE, ADDRESS_NOT_FOUND_MESSAGE,
        CHAIN_ID_PATH_PARAM, LIMIT_REACHED_MESSAGE,
    };

    mock! {
        AuthProvider {}
        #[async_trait]
        impl AuthorizationProviderByAddress for AuthProvider {
            async fn client_id_has_address_permission(
                &self,
                address: ethers::types::H160,
                client_id: &str,
            ) -> Result<bool, AuthorizationProviderError>;
        }
    }

    struct TestFixture {
        pub sponsor_repository: MockSponsorAddressConfigRepository,
        pub authorization_provider: MockAuthProvider,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        TestFixture {
            sponsor_repository: MockSponsorAddressConfigRepository::new(),
            authorization_provider: MockAuthProvider::new(),
        }
    }

    fn build_request(address: &str, chain_id: u64) -> Request<Body> {
        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let mut path_params: HashMap<String, String> = HashMap::new();
        path_params.insert(CHAIN_ID_PATH_PARAM.to_owned(), chain_id.to_string());

        let body = Body::Text(
            json!({
                "gas_pool_address": address,
            })
            .to_string(),
        );

        build_request_custom_auth(auth, body).with_path_parameters(path_params)
    }

    impl TestFixture {
        pub fn get_state(self) -> State<MockSponsorAddressConfigRepository, MockAuthProvider> {
            State {
                sponsor_repository: Arc::new(self.sponsor_repository),
                authorization_provider: self.authorization_provider,
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn invalid_address_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;

        let request = build_request("invalid_address", CHAIN_ID_FOR_MOCK_REQUESTS);

        let response = create_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert!(body.message.contains("Invalid H160 value"));
    }

    #[rstest]
    #[tokio::test]
    async fn already_exists_ok(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        fixture
            .sponsor_repository
            .expect_get_addresses()
            .once()
            .returning(|_, _, _| {
                Ok(vec![SponsorAddressConfig::GasPool {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    address: Address::default(),
                }])
            });

        let response = create_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(LIMIT_REACHED_MESSAGE, body.message);
    }

    #[rstest]
    #[tokio::test]
    async fn create_gas_pool_ok(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        fixture
            .sponsor_repository
            .expect_get_addresses()
            .returning(|_, _, _| Ok(vec![]));

        fixture
            .sponsor_repository
            .expect_put_address_gas_pool()
            .once()
            .returning(|_, _, _| Ok(()));

        let response = create_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::CREATED, response.status());
    }

    #[rstest]
    #[tokio::test]
    async fn create_gas_pool_same_address_ok(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(true));

        fixture
            .sponsor_repository
            .expect_get_addresses()
            .returning(|_, _, _| {
                Ok(vec![SponsorAddressConfig::GasPool {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                }])
            });

        let response = create_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::CREATED, response.status());
    }

    #[rstest]
    #[tokio::test]
    async fn client_id_not_allowed(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Ok(false));

        let response = create_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, ADDRESS_NOT_FOUND_CODE);
        assert_eq!(body.message, ADDRESS_NOT_FOUND_MESSAGE);
    }
}
