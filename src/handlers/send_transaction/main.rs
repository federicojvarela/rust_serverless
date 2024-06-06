mod config;
mod dtos;
mod model;

use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::secrets_manager::get_secrets_provider;
use dtos::request::SignedTransaction;
use ethers::providers::{Http, Middleware, MiddlewareError, Provider, ProviderError};
use model::SendTransactionResponse;
use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::EvmBlockchainProvider;
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::{
    http::utils::SUBMISSION_ERROR_CODE, lambda_main, lambda_structure::lambda_trait::Lambda,
    result::error::OrchestrationError,
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use std::sync::Arc;

type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;

pub struct State {
    blockchain_provider: BlockchainProviderObject,
}

pub struct SendTransaction;

#[async_trait]
impl Lambda for SendTransaction {
    type PersistedMemory = State;
    type InputBody = Event<SignedTransaction>;
    type Output = Event<SendTransactionResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let dynamo_db_client = get_dynamodb_client();
        let secrets_provider = get_secrets_provider().await;
        let config = ConfigLoader::load_default::<Config>();
        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let blockchain_provider = Arc::new(AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        )) as BlockchainProviderObject;

        Ok(State {
            blockchain_provider,
        })
    }

    async fn run(
        request: Self::InputBody,
        connections: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let chain_id = request.payload.transaction.get_chain_id();
        let endpoint = connections
            .blockchain_provider
            .get_evm_endpoint(chain_id, None)
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e).context("Unable to get endpoint")))?;

        let provider = Provider::<Http>::try_from(endpoint).map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Unable to instantiate provider"))
        })?;

        let bytes = hex::decode(request.payload.maestro_signature).map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Unable to decode signature"))
        })?;

        tracing::info!(chain_id,
            tx_hash = ?request.payload.transaction_hash,
            key_id = ?request.payload.key_id,
            "Sending tx in chain {} with hash {:?}", chain_id, request.payload.transaction_hash );

        let response = provider
            .send_raw_transaction(bytes.into())
            .await
            .map(|tx| {
                tracing::info!(chain_id,
                    tx_hash = ?tx.tx_hash(),
                    key_id = ?request.payload.key_id,
                    "Transaction sent in chain {} with hash {:?}", chain_id, tx.tx_hash());

                SendTransactionResponse::Submitted {
                    tx_hash: tx.tx_hash(),
                }
            })
            .or_else(|e| match get_submission_error_message(&e) {
                Some(message) => {
                    tracing::warn!(chain_id,
                        message,
                        tx_hash = ?request.payload.transaction_hash,
                        key_id = ?request.payload.key_id,
                        "Error sending tx in chain {} with hash {:?}. {}", chain_id, request.payload.transaction_hash, message);

                    Ok(SendTransactionResponse::NotSubmitted {
                        code: SUBMISSION_ERROR_CODE,
                        message,
                    })
                }

                None => Err(e),
            })
            .map_err(|e| {
                tracing::error!(chain_id,
                    error = ?e,
                    tx_hash = ?request.payload.transaction_hash,
                    key_id = ?request.payload.key_id,
                    "Error sending tx in chain {} with hash {:?}. {:?}", chain_id, request.payload.transaction_hash, e);

                OrchestrationError::from(anyhow!(e).context("Unable to send txn"))
            })?;

        Ok(request.context.create_new_event_from_current(response))
    }
}

fn get_submission_error_message(e: &ProviderError) -> Option<String> {
    if let Some(response) = e.as_error_response() {
        if response.code == SUBMISSION_ERROR_CODE {
            return Some(response.message.clone());
        }
    }
    None
}

lambda_main!(SendTransaction);
