mod config;
mod fees_calculator;
mod models;

use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request, RequestExt};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::model::gas_response::LambdaResponse;
use mpc_signature_sm::{
    blockchain::providers::{
        alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider, EvmBlockchainProvider,
        NewestBlock,
    },
    http::errors::unknown_error_response,
    http_lambda_main,
    lambda_structure::http_lambda_main::{HttpLambdaResponse, RequestExtractor},
    result::error::LambdaError,
    validations::http::supported_chain_id::validate_chain_id_is_supported,
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use std::cmp::min;
use std::sync::Arc;

const CHAIN_ID_PATH_PARAM: &str = "chain_id";

// Fee History Parameters
const BLOCK_COUNT: u64 = 5;
const MAX_BLOCK_COUNT: u64 = 100;

const NEWEST_BLOCK: NewestBlock = NewestBlock::Latest;
const REWARD_PERCENTILES: [f64; 3] = [0.0, 50.0, 100.0];

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
    get_historical_fees,
    [validate_chain_id_is_supported]
);

async fn get_historical_fees(
    request: Request,
    state: &State<impl EvmBlockchainProvider>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    let block_count = match request
        .query_string_parameters()
        .first("block_count")
        .unwrap_or("0")
        .parse()
        .unwrap_or(0)
    {
        0 => BLOCK_COUNT,
        value => min(value, MAX_BLOCK_COUNT),
    };

    let fee_history = state
        .provider
        .get_fee_history(chain_id, block_count, NEWEST_BLOCK, &REWARD_PERCENTILES)
        .await
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    let processed_fee_history = fees_calculator::get_historical_fees(fee_history)
        .map_err(|e| unknown_error_response(LambdaError::Unknown(e.into())))?;

    let fees = LambdaResponse {
        chain_id,
        max_priority_fee_per_gas: processed_fee_history.max_priority_fee_per_gas,
        max_fee_per_gas: processed_fee_history.max_fee_per_gas,
        gas_price: processed_fee_history.gas_price,
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
