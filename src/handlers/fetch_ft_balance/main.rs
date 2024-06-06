use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::requests::FTBalanceRequest;
use ethers::types::Address;
use lambda_http::Body;
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::authorization::{
    AuthorizationProviderByAddress, AuthorizationProviderByAddressImpl,
};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use mpc_signature_sm::{
    blockchain::providers::{
        alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider, EvmBlockchainProvider,
        FungibleTokenInfo,
    },
    http::errors::{
        unauthorized_error_response, unknown_error_response, validation_error_response,
    },
    lambda_structure::http_lambda_main::{
        CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
    },
    result::error::LambdaError,
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use reqwest::StatusCode;
use std::sync::Arc;
use validator::Validate;

mod config;
mod dtos;

pub const CHAIN_ID_PATH_PARAM: &str = "chain_id";
pub const ADDRESS_PATH_PARAM: &str = "address";

pub struct State<BP: EvmBlockchainProvider, AP: AuthorizationProviderByAddress> {
    pub authorization_provider: AP,
    pub blockchain_provider: BP,
    pub config: Config,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let secrets_provider = get_secrets_provider().await;
        let dynamo_db_client = get_dynamodb_client();
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
        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client,
        ));
        let authorization_provider = AuthorizationProviderByAddressImpl::new(keys_repository);

        State {
            authorization_provider,
            blockchain_provider,
            config,
        }
    },
    fetch_ft_balance,
    [validate_chain_id_is_supported, validate_content_type]
);

async fn fetch_ft_balance(
    request: Request,
    state: &State<impl EvmBlockchainProvider, impl AuthorizationProviderByAddress>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let app_client_id = request.extract_client_id()?;
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    let address: Address = request.extract_path_param(ADDRESS_PATH_PARAM)?;

    if !state
        .authorization_provider
        .client_id_has_address_permission(address, &app_client_id)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(anyhow!("{e}"))))?
    {
        return Err(unauthorized_error_response(None));
    }

    match request.body() {
        Body::Text(json_str) => {
            let body: FTBalanceRequest = serde_json::from_str(json_str).map_err(|e| {
                validation_error_response(e.to_string(), Some(LambdaError::Unknown(e.into())))
            })?;

            body.validate()
                .map_err(|e| validation_error_response(anyhow!(e).to_string(), None))?;

            let retval: FungibleTokenInfo = state
                .blockchain_provider
                .get_fungible_token_info(chain_id, address, body.contract_addresses)
                .await
                .map_err(|e| {
                    unknown_error_response(LambdaError::Unknown(anyhow!(
                        "Error getting balance: {:?}",
                        e.to_string()
                    )))
                })?;

            match serde_json::to_string(&retval) {
                Ok(body) => LambdaProxyHttpResponse {
                    status_code: StatusCode::OK,
                    body: Some(body),
                    ..LambdaProxyHttpResponse::default()
                }
                .try_into(),
                Err(e) => Err(unknown_error_response(LambdaError::Unknown(
                    anyhow!(e).context("Failed to parse string into json."),
                ))),
            }
        }
        _ => Err(validation_error_response(
            "Body wasn't a text type".to_owned(),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::{config::Config, fetch_ft_balance, State, ADDRESS_PATH_PARAM, CHAIN_ID_PATH_PARAM};
    use ana_tools::config_loader::ConfigLoader;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use ethers::types::{Address, Transaction, H160, U256};
    use http::StatusCode;
    use lambda_http::{
        aws_lambda_events::apigw::ApiGatewayProxyRequestContext, request::RequestContext, Body,
        Request, RequestExt,
    };
    use mockall::mock;
    use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
    use mpc_signature_sm::authorization::AuthorizationProviderError;
    use mpc_signature_sm::blockchain::providers::{
        BlockFeeQuery, BlockchainProviderError, EvmBlockchainProvider, FeeHistory,
        FungibleTokenInfo, FungibleTokenInfoDetail, FungibleTokenMetadataInfo, NativeTokenInfo,
        NewestBlock, NonFungibleTokenInfo, Pagination,
    };
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use mpc_signature_sm::http::errors::validation_error_response;
    use mpc_signature_sm::http::errors::SERVER_ERROR_CODE;
    use rstest::*;
    use serde_json::{json, Value};
    use std::{collections::HashMap, str::FromStr};

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
            blockchain_provider,
            config,
            authorization_provider,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_missing_chain_id_param(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            None,
            None,
            body,
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
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("aaaa".to_string()),
            None,
            body,
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
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            None,
            body,
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
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("".to_string()),
            body,
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
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("aaaa".to_string()),
            body,
            StatusCode::BAD_REQUEST,
            validation_error_response("address with wrong type in request path".to_string(), None)
                .body(),
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_incorrect_body(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(true));

        let body = json!({ "contract_addresses": [] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0".to_string()),
            body,
            StatusCode::BAD_REQUEST,
            "contract_addresses: Validation error: length",
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_client_not_authorized(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(0);

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(false));

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0".to_string()),
            body,
            StatusCode::UNAUTHORIZED,
            "",
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_blockchain_provider_error(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(1)
            .returning(|_, _, _| {
                Err(BlockchainProviderError::Unknown(anyhow!(
                    "Unable to get endpoint"
                )))
            });

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(true));

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0".to_string()),
            body,
            StatusCode::INTERNAL_SERVER_ERROR,
            SERVER_ERROR_CODE,
        )
        .await
    }

    #[rstest]
    #[tokio::test]
    async fn handle_fetch_ft_token_ok(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_fungible_token_metadata()
            .times(0);

        fixture
            .blockchain_provider
            .expect_get_fungible_token_info()
            .times(1)
            .returning(|_, _, _| {
                Ok(FungibleTokenInfo {
                    data: vec![FungibleTokenInfoDetail {
                        contract_address: H160::from_str(
                            "0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0",
                        )
                        .unwrap(),
                        name: "".to_string(),
                        symbol: "".to_string(),
                        balance: "".to_string(),
                        logo: "".to_string(),
                        decimals: "".to_string(),
                    }],
                    errors: vec![],
                })
            });

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .once()
            .returning(|_, _| Ok(true));

        let body = json!({ "contract_addresses": vec![ADDRESS_FOR_MOCK_REQUESTS] }).to_string();

        assert_expected_body_and_status(
            fixture,
            Some("1".to_string()),
            Some("0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0".to_string()),
            body,
            StatusCode::OK,
            "[{\"contract_address\":\"0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0\",\"balance\":\"\",\"name\":\"\",\"symbol\":\"\",\"logo\":\"\",\"decimals\":\"\"}]",
        )
        .await
    }

    #[tokio::test]
    async fn validate_can_construct_right_json_body() {
        let address = H160::from_str("0x3f5ce5fbfe3e9af3971dd833d26ba9b5c936f0be").unwrap();
        let contract_addresses = [
            H160::from_str("0xc11aab3e363e3ca9ff5e7e82c6298004c39b7ec2").unwrap(),
            H160::from_str("0xc11aab3e363e3ca9ff5e7e82c6298004c39b7ec2").unwrap(),
        ];

        let contract_addresses = contract_addresses
            .iter()
            .map(|address| format!("{:?}", address))
            .collect::<Vec<String>>();

        let expected = "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"alchemy_getTokenBalances\",\"params\":[\"0x3f5ce5fbfe3e9af3971dd833d26ba9b5c936f0be\",[\"0xc11aab3e363e3ca9ff5e7e82c6298004c39b7ec2\",\"0xc11aab3e363e3ca9ff5e7e82c6298004c39b7ec2\"]]}";

        let body = json!(
        {
            "jsonrpc" : "2.0",
            "id": 1,
            "method" : "alchemy_getTokenBalances",
            "params" : [format!("{:?}",address), contract_addresses],
        });

        assert_eq!(expected, body.to_string());
    }

    fn request_with_params(
        chain_id: Option<String>,
        address: Option<String>,
        body: String,
    ) -> Request {
        let mut path_params: HashMap<String, String> = [].into();

        if let Some(chain_id) = chain_id {
            path_params.insert(CHAIN_ID_PATH_PARAM.to_string(), chain_id);
        }

        if let Some(address) = address {
            path_params.insert(ADDRESS_PATH_PARAM.to_string(), address);
        }

        let authorizer: HashMap<String, Value> = HashMap::from([(
            "claims".to_string(),
            json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS }),
        )]);

        let request_context = RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext {
            authorizer,
            ..ApiGatewayProxyRequestContext::default()
        });

        Request::new(Body::Text(body))
            .with_path_parameters::<HashMap<_, _>>(path_params)
            .with_request_context(request_context)
    }

    async fn assert_expected_body_and_status(
        fixture: TestFixture,
        chain_id: Option<String>,
        address: Option<String>,
        body: String,
        expected_status: StatusCode,
        expected_body_substring: &str,
    ) {
        let request = request_with_params(chain_id, address, body);

        let response = fetch_ft_balance(
            request,
            &State {
                authorization_provider: fixture.authorization_provider,
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
