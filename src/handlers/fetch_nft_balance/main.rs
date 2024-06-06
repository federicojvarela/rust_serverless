mod config;
mod dtos;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::requests::NftBalanceRequest;
use ethers::types::Address;
use http::StatusCode;
use lambda_http::{run, Body, Error, Request};
use mpc_signature_sm::authorization::{
    AuthorizationProviderByAddress, AuthorizationProviderByAddressImpl,
};
use mpc_signature_sm::blockchain::config::Config as BlockchainConfig;
use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::{EvmBlockchainProvider, Pagination};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{
    unauthorized_error_response, unknown_error_response, validation_error_response,
};
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use std::sync::Arc;
use tower::service_fn;
use validator::Validate;

use crate::config::Config;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::result::error::LambdaError;

const CHAIN_ID_PATH_PARAM: &str = "chain_id";
const ADDRESS_PATH_PARAM: &str = "address";

pub struct State<BP: EvmBlockchainProvider, A: AuthorizationProviderByAddress> {
    pub evm_blockchain_provider: BP,
    pub authorization_provider: A,
    pub config: Config,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamo_db_client = get_dynamodb_client();
        let secrets_provider = get_secrets_provider().await;

        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));
        let evm_blockchain_provider = AlchemyEvmBlockchainProvider::new(
            ConfigLoader::load_default::<BlockchainConfig>(),
            secrets_provider,
            cache_repository,
        );
        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client.clone(),
        ));
        let authorization_provider = AuthorizationProviderByAddressImpl::new(keys_repository);

        State {
            evm_blockchain_provider,
            authorization_provider,
            config,
        }
    },
    fetch_nft_balance,
    [validate_chain_id_is_supported, validate_content_type]
);

async fn fetch_nft_balance(
    request: Request,
    state: &State<impl EvmBlockchainProvider, impl AuthorizationProviderByAddress>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let client_id = request.extract_client_id()?;
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    let address: Address = request.extract_path_param(ADDRESS_PATH_PARAM)?;

    if !state
        .authorization_provider
        .client_id_has_address_permission(address, &client_id)
        .await
        .map_err(move |e| {
            unknown_error_response(LambdaError::Unknown(
                anyhow!(e).context("Error checking permissions"),
            ))
        })?
    {
        return Err(unauthorized_error_response(None));
    }

    match request.body() {
        Body::Text(json_str) => {
            let body: NftBalanceRequest = serde_json::from_str(json_str).map_err(|e| {
                validation_error_response(e.to_string(), Some(LambdaError::Unknown(e.into())))
            })?;

            body.validate()
                .map_err(|e| validation_error_response(anyhow!(e).to_string(), None))?;

            let token_info = state
                .evm_blockchain_provider
                .get_non_fungible_token_info(
                    chain_id,
                    address,
                    body.contract_addresses,
                    body.pagination.map(Pagination::from).unwrap_or_default(),
                )
                .await
                .map_err(|e| unknown_error_response(e.into()))?;

            let response_body = serde_json::to_string(&token_info).map_err(|e| {
                unknown_error_response(LambdaError::Unknown(
                    anyhow!(e).context("Error serializing response"),
                ))
            })?;

            LambdaProxyHttpResponse {
                status_code: StatusCode::OK,
                body: Some(response_body),
                ..LambdaProxyHttpResponse::default()
            }
            .try_into()
        }
        _ => Err(validation_error_response(
            "Body wasn't a text type".to_owned(),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dtos::requests::{NftBalanceRequest, PaginationRequest};
    use crate::{fetch_nft_balance, State};

    use anyhow::anyhow;
    use async_trait::async_trait;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CONTRACT_ADDRESS_FOR_MOCK_NFT_REQUESTS,
    };
    use ethers::types::{Address, Transaction, H160, U256};
    use http::{Request, StatusCode};
    use lambda_http::aws_lambda_events::apigw::ApiGatewayProxyRequestContext;
    use lambda_http::request::RequestContext;
    use lambda_http::{Body, RequestExt};
    use mockall::mock;
    use mockall::predicate::eq;
    use mpc_signature_sm::authorization::AuthorizationProviderByAddress;
    use mpc_signature_sm::authorization::AuthorizationProviderError;
    use mpc_signature_sm::blockchain::providers::*;
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use rstest::{fixture, rstest};
    use serde_json::{json, Value};
    use std::{collections::HashMap, str::FromStr};

    const CHAIN_ID: &str = "1";
    const CLIENT_ID: &str = "hbfeiwuh2i3h21312083u12312soidj";

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
        pub evm_blockchain_provider: MockBlockchainProvider,
        pub config: Config,
        pub authorization_provider: MockAuthProvider,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            evm_blockchain_provider: MockBlockchainProvider::new(),
            config: Config {
                keys_table_name: "keys".to_owned(),
                cache_table_name: "cache".to_owned(),
            },
            authorization_provider: MockAuthProvider::new(),
        }
    }

    fn build_request(
        client_id: Option<&str>,
        chain_id: Option<&str>,
        address: Option<&str>,
        body: Option<String>,
    ) -> Request<Body> {
        let request = if let Some(body) = body {
            Request::new(body.into())
        } else {
            Request::default()
        };
        let mut path_parameters = HashMap::new();

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

        if let Some(chain_id) = chain_id {
            path_parameters.insert("chain_id".to_owned(), chain_id.to_string());
        }

        if let Some(address) = address {
            path_parameters.insert("address".to_owned(), address.to_string());
        }

        request
            .with_path_parameters(path_parameters)
            .with_request_context(request_context)
    }

    fn build_valid_body() -> Option<String> {
        Some(
            serde_json::to_string(&NftBalanceRequest {
                contract_addresses: vec![
                    H160::from_str(CONTRACT_ADDRESS_FOR_MOCK_NFT_REQUESTS).unwrap()
                ],
                pagination: Some(PaginationRequest {
                    page_size: Some(10),
                    page_key: None,
                }),
            })
            .unwrap(),
        )
    }

    #[rstest]
    #[tokio::test]
    async fn authorization_header_missing(fixture: TestFixture) {
        let request: Request<Body> = build_request(
            None,
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            build_valid_body(),
        );
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::UNAUTHORIZED, error.status());
    }

    #[rstest]
    #[tokio::test]
    async fn chain_id_path_param_missing(fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            None,
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            build_valid_body(),
        );
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("chain_id not found in request path"));
    }

    #[rstest]
    #[tokio::test]
    async fn chain_id_path_param_wrong_type(fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some("invalid"),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            build_valid_body(),
        );
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("chain_id with wrong type in request path"));
    }

    #[rstest]
    #[tokio::test]
    async fn address_path_param_missing(fixture: TestFixture) {
        let request: Request<Body> =
            build_request(Some(CLIENT_ID), Some(CHAIN_ID), None, build_valid_body());
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("address not found in request path"));
    }

    #[rstest]
    #[tokio::test]
    async fn address_path_param_wrong_type(fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some("25"),
            build_valid_body(),
        );
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("address with wrong type in request path"));
    }

    #[rstest]
    #[tokio::test]
    async fn authorization_provider_failure(mut fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            None,
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Err(AuthorizationProviderError::Unknown(anyhow!("timeout!"))));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, error.status());
    }

    #[rstest]
    #[tokio::test]
    async fn address_does_not_belong_to_client_id(mut fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            None,
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(false));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::UNAUTHORIZED, error.status());
    }

    #[rstest]
    #[tokio::test]
    async fn invalid_body_type(mut fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            None,
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error.body().to_string().contains("Body wasn't a text type"));
    }

    #[rstest]
    #[tokio::test]
    async fn invalid_body(mut fixture: TestFixture) {
        let body = json!({
            "contracts": [],
            "paging": {}
        })
        .to_string();

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            Some(body),
        );
        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error.body().to_string().contains("missing field"));
    }

    #[rstest]
    #[tokio::test]
    async fn blockchain_provider_call_failure(mut fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            build_valid_body(),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        fixture
            .evm_blockchain_provider
            .expect_get_non_fungible_token_info()
            .once()
            .returning(|_, _, _, _| Err(BlockchainProviderError::Unknown(anyhow!("timeout"))));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, error.status());
    }

    #[rstest]
    #[case::zero_addresses(vec![])]
    #[case::too_many_addresses((1..47).map(|_| Address::random()).collect::<Vec<Address>>())]
    #[tokio::test]
    async fn contract_addresses_not_in_range(
        mut fixture: TestFixture,
        #[case] contract_addresses: Vec<Address>,
    ) {
        let body = json!({ "contract_addresses": contract_addresses }).to_string();
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            Some(body),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("contract_addresses: Validation error: length"));
    }

    #[rstest]
    #[case::page_size_less_than_min(0)]
    #[case::page_size_more_than_max(101)]
    #[tokio::test]
    async fn page_size_not_in_range(mut fixture: TestFixture, #[case] page_size: u32) {
        let body = json!({
            "contract_addresses": vec![H160::random()],
            "pagination":{
                "page_size": page_size
            }
        })
        .to_string();

        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            Some(body),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        let error = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, error.status());
        assert!(error
            .body()
            .to_string()
            .contains("pagination.page_size: Validation error: range"));
    }

    #[rstest]
    #[tokio::test]
    async fn nft_balance_ok(mut fixture: TestFixture) {
        let request: Request<Body> = build_request(
            Some(CLIENT_ID),
            Some(CHAIN_ID),
            Some(ADDRESS_FOR_MOCK_REQUESTS),
            build_valid_body(),
        );

        fixture
            .authorization_provider
            .expect_client_id_has_address_permission()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CLIENT_ID.to_owned()),
            )
            .once()
            .returning(|_, _| Ok(true));

        fixture
            .evm_blockchain_provider
            .expect_get_non_fungible_token_info()
            .once()
            .returning(|_, _, _, _| {
                Ok(NonFungibleTokenInfo {
                    tokens: vec![NonFungibleTokenInfoDetail {
                        contract_address: Address::from_str(CONTRACT_ADDRESS_FOR_MOCK_NFT_REQUESTS)
                            .unwrap(),
                        name: "Bored Apes".to_owned(),
                        symbol: "BYC".to_owned(),
                        balance: "1".to_owned(),
                        metadata: NonFungibleTokenInfoMetadata {
                            name: "Some Ape".to_owned(),
                            description: "".to_owned(),
                            image: "https://...".to_owned(),
                            attributes: vec![],
                        },
                    }],
                    pagination: Pagination {
                        page_size: 10,
                        page_key: Some("qowhohaohqwoeqweouqhw12321".to_owned()),
                    },
                })
            });

        let response = fetch_nft_balance(
            request,
            &State {
                evm_blockchain_provider: fixture.evm_blockchain_provider,
                config: fixture.config,
                authorization_provider: fixture.authorization_provider,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        let expected_response_body = json!({
            "tokens": [{
                "contract_address": CONTRACT_ADDRESS_FOR_MOCK_NFT_REQUESTS,
                "name": "Bored Apes",
                "symbol": "BYC",
                "balance": "1",
                "metadata":  {
                    "name": "Some Ape",
                    "description": "",
                    "image": "https://...",
                    "attributes": [],
                },
            }],
            "pagination":  {
                "page_size": 10,
                "page_key": "qowhohaohqwoeqweouqhw12321",
            },
        });
        let actual_response_body: Value = serde_json::from_str(response.body()).unwrap();

        assert_eq!(StatusCode::OK, response.status());
        assert_eq!(expected_response_body, actual_response_body);
    }
}
