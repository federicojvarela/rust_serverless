mod config;
use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::blockchain::gas_fees::prediction::get_predicted_fees;
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::model::gas_response::LambdaResponse;
use mpc_signature_sm::{
    blockchain::providers::{
        alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider, EvmBlockchainProvider,
    },
    http::errors::unknown_error_response,
    http_lambda_main,
    lambda_structure::http_lambda_main::{HttpLambdaResponse, RequestExtractor},
    result::error::LambdaError,
    validations::http::supported_chain_id::validate_chain_id_is_supported,
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use std::sync::Arc;

const CHAIN_ID_PATH_PARAM: &str = "chain_id";

pub struct State<T: EvmBlockchainProvider> {
    pub provider: T,
}

http_lambda_main!(
    {
        let dynamo_db_client = get_dynamodb_client();
        let secrets_provider = get_secrets_provider().await;
        let config = ConfigLoader::load_default::<Config>();

        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let provider = AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        );

        State { provider }
    },
    get_gas_price_prediction,
    [validate_chain_id_is_supported]
);

async fn get_gas_price_prediction(
    request: Request,
    state: &State<impl EvmBlockchainProvider>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    let fees = get_predicted_fees(&state.provider, chain_id)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e)))?;

    let fees = LambdaResponse {
        chain_id,
        max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
        max_fee_per_gas: fees.max_fee_per_gas,
        gas_price: fees.gas_price,
    };

    let body = serde_json::to_string(&fees)
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        body: Some(body),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}
