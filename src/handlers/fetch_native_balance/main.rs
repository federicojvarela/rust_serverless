mod config;

use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use ethers::types::Address;
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
use mpc_signature_sm::authorization::AuthorizationProviderByAddressImpl;
use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::EvmBlockchainProvider;
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{unauthorized_error_response, unknown_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use reqwest::StatusCode;
use std::sync::Arc;

pub const CHAIN_ID_PATH_PARAM: &str = "chain_id";
pub const ADDRESS_PATH_PARAM: &str = "address";

pub struct State<BP: EvmBlockchainProvider, AU: AuthorizationProviderByAddress> {
    pub config: Config,
    pub auth_provider: AU,
    pub blockchain_provider: BP,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamo_db_client = get_dynamodb_client();
        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client.clone(),
        ));
        let auth_provider = AuthorizationProviderByAddressImpl::new(keys_repository);
        let secrets_provider = get_secrets_provider().await;
        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();

        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));
        let blockchain_provider = AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        );

        State {
            config,
            auth_provider,
            blockchain_provider,
        }
    },
    fetch_native_token,
    [validate_chain_id_is_supported]
);

async fn fetch_native_token(
    request: Request,
    state: &State<impl EvmBlockchainProvider, impl AuthorizationProviderByAddress>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    let address: Address = request.extract_path_param(ADDRESS_PATH_PARAM)?;

    let app_client_id = request.extract_client_id()?;

    if !state
        .auth_provider
        .client_id_has_address_permission(address, &app_client_id)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(
                anyhow!(e).context("Error checking permissions"),
            ))
        })?
    {
        return Err(unauthorized_error_response(None));
    }

    let token_info = state
        .blockchain_provider
        .get_native_token_info(chain_id, address)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(anyhow!("{e}"))))?;

    let body = serde_json::to_string(&token_info)
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
    use crate::config::Config;
    use crate::{fetch_native_token, State, ADDRESS_PATH_PARAM, CHAIN_ID_PATH_PARAM};
    use ana_tools::config_loader::ConfigLoader;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use ethers::types::{Address, Transaction, U256};
    use http::StatusCode;
    use lambda_http::aws_lambda_events::apigw::ApiGatewayProxyRequestContext;
    use lambda_http::request::RequestContext;
    use lambda_http::{Body, Request, RequestExt};
    use mockall::mock;
    use mockall::predicate::eq;
    use mpc_signature_sm::authorization::errors::AuthorizationProviderError;
    use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
    use mpc_signature_sm::blockchain::providers::{
        BlockFeeQuery, BlockchainProviderError, EvmBlockchainProvider, FeeHistory,
        FungibleTokenInfo, FungibleTokenMetadataInfo, NativeTokenInfo, NewestBlock,
        NonFungibleTokenInfo, Pagination,
    };
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use mpc_signature_sm::http::errors::{validation_error_response, SERVER_ERROR_CODE};
    use rstest::*;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::str::FromStr;

    mock! {
        AuthProvider {}
        #[async_trait]
        impl AuthorizationProviderByAddress for AuthProvider {
            async fn client_id_has_address_permission(
                &self,
                address: Address,
                client_id: &str,
            ) -> Result<bool, AuthorizationProviderError>;
        }
    }

    mock! {
        BlockchainProvider {}
        #[async_trait]
        impl EvmBlockchainProvider for BlockchainProvider {

            async fn get_evm_endpoint(
                &self,
                chain_id: u64,
                endpoint_prefix: Option<String>,
            ) -> Result<String, BlockchainProviderError>;

            async fn get_native_token_info(
                &self,
                chain_id: u64,
                address: Address,
            ) -> Result<NativeTokenInfo, BlockchainProviderError>;

            async fn get_non_fungible_token_info(
                &self,
                chain_id: u64,
                address: Address,
                contract_addresses: Vec<Address>,
                pagination: Pagination,
            ) -> Result<NonFungibleTokenInfo, BlockchainProviderError>;

            async fn get_fungible_token_info(
                &self,
                chain_id: u64,
                address: Address,
                contract_addresses: Vec<Address>,
            ) -> Result<FungibleTokenInfo, BlockchainProviderError>;

            async fn get_fungible_token_metadata(
                &self,
                chain_id: u64,
                address: Address,
            ) -> Result<FungibleTokenMetadataInfo, BlockchainProviderError>;

            async fn get_fee_history<'percentiles>(
                &self,
                chain_id: u64,
                block_count: u64,
                newest_block: NewestBlock,
                reward_percentiles: &'percentiles [f64],
            ) -> Result<FeeHistory, BlockchainProviderError>;

            async fn tx_status_succeed(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<bool, BlockchainProviderError>;

            async fn get_tx_by_hash(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<Option<Transaction>, BlockchainProviderError>;

            async fn get_tx_receipt(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<Option<ethers::types::TransactionReceipt>, BlockchainProviderError>;

            async fn get_current_nonce(
                &self,
                chain_id: u64,
                address: Address
            ) -> Result<U256, BlockchainProviderError>;

            async fn get_fees_from_pending(
                &self,
                chain_id: u64,
            ) -> Result<BlockFeeQuery, BlockchainProviderError>;
        }
    }

    struct TestFixture {
        pub config: Config,
        pub blockchain_provider: MockBlockchainProvider,
        pub authorization_provider: MockAuthProvider,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = ConfigLoader::load_test::<Config>();
        let blockchain_provider = MockBlockchainProvider::new();
        let authorization_provider = MockAuthProvider::new();
        TestFixture {
            config,
            blockchain_provider,
            authorization_provider,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_missing_chain_id_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(0);

        assert_expected_body_and_status(
            fixture,
            None,
            None,
            StatusCode::BAD_REQUEST,
            validation_error_response("chain_id not found in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_incorrect_chain_id_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(0);

        assert_expected_body_and_status(
            fixture,
            Some("aaaa".to_string()),
            None,
            StatusCode::BAD_REQUEST,
            validation_error_response("chain_id with wrong type in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_missing_address_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(0);
        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            None,
            StatusCode::BAD_REQUEST,
            validation_error_response("address not found in request path".to_string(), None).body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_empty_address_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(0);

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("".to_string()),
            StatusCode::BAD_REQUEST,
            validation_error_response("address with wrong type in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_incorrect_address_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(0);

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("aaaa".to_string()),
            StatusCode::BAD_REQUEST,
            validation_error_response("address with wrong type in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_blockchain_provider_error(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(1)
            .returning(|_, _| {
                Err(BlockchainProviderError::Unknown(anyhow!(
                    "Unable to get endpoint"
                )))
            });

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(true));

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            SERVER_ERROR_CODE,
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_fetch_native_token_ok(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_native_token_info()
            .times(1)
            .returning(|_, _| {
                Ok(NativeTokenInfo {
                    chain_id: 1,
                    balance: U256::from(100000059).to_string(),
                    symbol: "ETH".to_string(),
                    name: "Ether".to_string(),
                })
            });

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(true));

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            StatusCode::OK,
            "{\"name\":\"Ether\",\"symbol\":\"ETH\",\"chain_id\":1,\"balance\":\"100000059\"}",
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn authorization_header_missing(fixture: TestFixture) {
        let request = request_with_params(
            Some("1".to_string()),
            Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            None,
        );

        let response = fetch_native_token(
            request,
            &State {
                auth_provider: fixture.authorization_provider,
                blockchain_provider: fixture.blockchain_provider,
                config: fixture.config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::UNAUTHORIZED, response.status());
    }

    #[rstest]
    #[tokio::test]
    async fn authorization_provider_failure(mut fixture: TestFixture) {
        let request = request_with_params(
            Some("1".to_string()),
            Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            Some(CLIENT_ID_FOR_MOCK_REQUESTS),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS.to_owned()),
            )
            .once()
            .returning(|_, _| Err(AuthorizationProviderError::Unknown(anyhow!("timeout!"))));

        let response = fetch_native_token(
            request,
            &State {
                auth_provider: fixture.authorization_provider,
                blockchain_provider: fixture.blockchain_provider,
                config: fixture.config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
    }

    #[rstest]
    #[tokio::test]
    async fn address_does_not_belong_to_client_id(mut fixture: TestFixture) {
        let request = request_with_params(
            Some("1".to_string()),
            Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            Some(CLIENT_ID_FOR_MOCK_REQUESTS),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID_FOR_MOCK_REQUESTS.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(false));

        let response = fetch_native_token(
            request,
            &State {
                auth_provider: fixture.authorization_provider,
                blockchain_provider: fixture.blockchain_provider,
                config: fixture.config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::UNAUTHORIZED, response.status());
    }

    fn request_with_params(
        chain_id: Option<String>,
        address: Option<String>,
        client_id: Option<&str>,
    ) -> Request {
        let mut path_params: HashMap<String, String> = [].into();

        if let Some(chain_id) = chain_id {
            path_params.insert(CHAIN_ID_PATH_PARAM.to_string(), chain_id);
        }

        if let Some(address) = address {
            path_params.insert(ADDRESS_PATH_PARAM.to_string(), address);
        }

        let request =
            Request::new(Body::Binary(vec![])).with_path_parameters::<HashMap<_, _>>(path_params);

        let request_context = if let Some(client_id) = client_id {
            let authorizer: HashMap<String, Value> =
                HashMap::from([("claims".to_string(), json!({ "client_id": client_id }))]);

            RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext {
                authorizer,
                ..ApiGatewayProxyRequestContext::default()
            })
        } else {
            RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext::default())
        };

        request.with_request_context(request_context)
    }

    async fn assert_expected_body_and_status(
        fixture: TestFixture,
        chain_id: Option<String>,
        address: Option<String>,
        expected_status: StatusCode,
        expected_body_substring: &str,
    ) {
        let request = request_with_params(chain_id, address, Some(CLIENT_ID_FOR_MOCK_REQUESTS));

        let response = fetch_native_token(
            request,
            &State {
                auth_provider: fixture.authorization_provider,
                blockchain_provider: fixture.blockchain_provider,
                config: fixture.config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await;

        let response = match response {
            Ok(response) => response,
            Err(response) => response,
        };

        assert_eq!(response.status(), expected_status);
        let body = response.body().as_str();
        assert!(body.contains(expected_body_substring));
    }
}
