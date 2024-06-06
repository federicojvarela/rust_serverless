use crate::blockchain::config::{ChainEndpointInformation, Config};
use crate::blockchain::providers::{
    alchemy::dtos::{
        fts::AlchemyGetFTsResponse, fts::AlchemyGetMetadataErrorResponse,
        fts::AlchemyGetMetadataResponse, nfts::AlchemyGetNftsResponse, AlchemyRPCResponse,
    },
    BlockchainProviderError, EvmBlockchainProvider, FungibleTokenInfo, FungibleTokenInfoDetail,
    NativeTokenInfo, NonFungibleTokenInfo, NonFungibleTokenInfoDetail, Pagination, Result,
    TokenError,
};
use crate::blockchain::providers::{
    BlockFeeQuery, FeeHistory, FungibleTokenMetadata, FungibleTokenMetadataInfo, NewestBlock,
};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use ethers::prelude::RetryClient;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, BlockNumber, Transaction, H160, H256, U256};
use model::cache::{DataType, GenericJsonCache};
use repositories::cache::{CacheRepository, CacheRepositoryError};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use secrets_provider::SecretsProvider;
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const MIN_RETRY_INTERVAL: u64 = 30;
const MAX_RETRY_INTERVAL: u64 = 200;
const CACHE_DURATION: u64 = 86400; // one day

pub struct AlchemyEvmBlockchainProvider<S: SecretsProvider, R: CacheRepository> {
    config: Config,
    secrets_provider: S,
    http_client: reqwest_middleware::ClientWithMiddleware,
    cache_repository: Arc<R>,
}

impl<S: SecretsProvider + Send + Sync, R: CacheRepository + Sync + Send>
    AlchemyEvmBlockchainProvider<S, R>
{
    pub fn new(config: Config, secrets_provider: S, cache_repository: Arc<R>) -> Self {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                Duration::from_millis(MIN_RETRY_INTERVAL),
                Duration::from_millis(MAX_RETRY_INTERVAL),
            )
            .build_with_max_retries(MAX_RETRIES);
        let http_client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            config,
            secrets_provider,
            http_client,
            cache_repository,
        }
    }

    async fn get_provider_for(&self, chain_id: u64) -> Result<Provider<RetryClient<Http>>> {
        let endpoint = self.get_evm_endpoint(chain_id, None).await?;

        Provider::new_client(
            &endpoint,
            MAX_RETRIES,
            Duration::from_millis(MIN_RETRY_INTERVAL).as_millis() as u64,
        )
        .map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Error building provider"))
        })
    }

    async fn save_data_to_cache(
        &self,
        data: Option<FungibleTokenMetadata>,
        data_type: DataType,
        key: String,
    ) {
        // if there is metadata we store it in the data array
        if let Some(metadata) = data {
            let ttl = Utc::now() + Duration::from_secs(CACHE_DURATION);
            let cache_item = GenericJsonCache {
                pk: data_type,
                sk: key.clone(),
                created_at: Utc::now(),
                data: match serde_json::to_value::<FungibleTokenMetadata>(metadata) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!("Error serializing metadata: {e}");
                        return;
                    }
                },
                expires_at: ttl.timestamp(),
            };

            // save the metadata into the cache using the set_item function from cache_repository
            match self.cache_repository.set_item(cache_item).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!("Error saving metadata to cache: {e}");
                }
            }
        }
    }
}

#[async_trait]
impl<S, R> EvmBlockchainProvider for AlchemyEvmBlockchainProvider<S, R>
where
    S: SecretsProvider + Send + Sync,
    R: CacheRepository + Sync + Send,
{
    async fn get_evm_endpoint(
        &self,
        chain_id: u64,
        endpoint_prefix: Option<String>,
    ) -> Result<String> {
        let ChainEndpointInformation {
            endpoint,
            secret_name,
        } = self
            .config
            .evm_chain_endpoint_information_for(chain_id)
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context(format!("Error getting endpoint for chain_id: {chain_id}")),
                )
            })?;

        if let Some(secret_name) = secret_name {
            let endpoint_prefix_path = endpoint_prefix.unwrap_or_default();
            let api_key: String = self
                .secrets_provider
                .find(secret_name.as_str())
                .await
                .map_err(|e| {
                    BlockchainProviderError::Unknown(anyhow!(
                        "There was an error initializing getting the Alchemy API KEY.\nSecret {secret_name}\nError: {e}"
                    ))
                })?
                .ok_or_else(|| {
                    BlockchainProviderError::Unknown(anyhow!(
                        "Alchemy API Key not found in Secrets Manager. Secret {secret_name}"
                    ))
                })?.reveal();

            Ok(format!("{endpoint}{endpoint_prefix_path}/v2/{api_key}"))
        } else {
            Ok(endpoint)
        }
    }

    async fn get_native_token_info(
        &self,
        chain_id: u64,
        address: Address,
    ) -> Result<NativeTokenInfo> {
        let provider = self.get_provider_for(chain_id).await?;
        let balance = provider.get_balance(address, None).await.map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!("Error getting balance: {e:?}"))
        })?;

        let native_token = self
            .config
            .get_chain_native_token(chain_id)
            .map_err(BlockchainProviderError::Unknown)?;

        Ok(NativeTokenInfo {
            balance: balance.to_string(),
            chain_id,
            symbol: native_token.symbol,
            name: native_token.name,
        })
    }

    async fn get_non_fungible_token_info(
        &self,
        chain_id: u64,
        address: Address,
        contract_addresses: Vec<Address>,
        pagination: Pagination,
    ) -> Result<NonFungibleTokenInfo> {
        let endpoint = self
            .get_evm_endpoint(chain_id, Some("/nft".to_owned()))
            .await?;

        let contract_addresses = contract_addresses
            .iter()
            .map(|address| format!("{:?}", address))
            .collect::<Vec<String>>()
            .join(",");

        let page_key = pagination.page_key.unwrap_or("null".to_owned());

        let response = self
            .http_client
            .get(format!("{endpoint}/getNFTs"))
            .query(&[
                ("owner", format!("{:?}", address)),
                ("contractAddresses[]", contract_addresses),
                ("withMetadata", true.to_string()),
                ("pageKey", page_key),
                ("pageSize", pagination.page_size.to_string()),
            ])
            .send()
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context(format!("Error querying NFTs for address {:?}", address)),
                )
            })?;

        let response_status = response.status();
        let response_body = response.text().await.map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Error obtaining http response"))
        })?;

        if !response_status.is_success() {
            return Err(BlockchainProviderError::Unknown(anyhow!(
                "Alchemy getNFTs failed with status {}. Response: {}",
                response_status,
                response_body
            )));
        }

        let response: AlchemyGetNftsResponse =
            serde_json::from_str(&response_body).map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context(format!(
                    "Error deserializing Alchemy getNFTs endpoint response for address {:?}",
                    address
                )))
            })?;

        let tokens = response
            .owned_nfts
            .into_iter()
            .map(NonFungibleTokenInfoDetail::try_from)
            .collect::<std::result::Result<Vec<NonFungibleTokenInfoDetail>, anyhow::Error>>()
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Error parsing Alchemy getNFTs response"),
                )
            })?;

        Ok(NonFungibleTokenInfo {
            tokens,
            pagination: Pagination {
                page_size: pagination.page_size,
                page_key: response.page_key,
            },
        })
    }

    async fn get_fungible_token_info(
        &self,
        chain_id: u64,
        address: Address,
        contract_addresses: Vec<Address>,
    ) -> Result<FungibleTokenInfo> {
        let endpoint = self.get_evm_endpoint(chain_id, None).await?;

        let contract_addresses = contract_addresses
            .iter()
            .map(|address| format!("{:?}", address))
            .collect::<Vec<String>>();

        let body = json!(
        {
          "jsonrpc" : "2.0",
          "id": chain_id.to_string(),
          "method" : "alchemy_getTokenBalances",
          "params" : [format!("{:?}", address.clone()), contract_addresses],
        });

        let response = self
            .http_client
            .post(endpoint.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context(format!("Error querying FTs for address {:?}", address)),
                )
            })?;

        let response_status = response.status();
        let response_body = response.text().await.map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Error obtaining http response"))
        })?;

        if !response_status.is_success() {
            return Err(BlockchainProviderError::Unknown(anyhow!(
                "Alchemy getFTs failed with status {}. Response: {}",
                response_status,
                response_body
            )));
        }

        let response: AlchemyGetFTsResponse =
            serde_json::from_str(&response_body).map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context(format!(
                    "Error deserializing Alchemy getFTs endpoint response for address {:?}",
                    address
                )))
            })?;

        let mut retval = FungibleTokenInfo {
            data: vec![],
            errors: vec![],
        };

        for token in response.result.token_balances.into_iter() {
            let contrat_address = H160::from_str(&token.contract_address).map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context("Error parsing address"))
            })?;
            let key = format!(
                "CONTRACT_ADDRESS#{}#CHAIN_ID#{}",
                token.contract_address, chain_id
            );
            // Add a check to verify if the metadata is already in the cache using the function get_item from cache_repository.
            // if the metadata is already in the cache we don't need to query it again.
            let metadata: Option<FungibleTokenMetadata>;
            let error_token: Option<TokenError>;
            match self
                .cache_repository
                .get_item(&key, DataType::FtMetadata)
                .await
            {
                Ok(cache_entry) => {
                    match serde_json::from_value::<FungibleTokenMetadata>(cache_entry.data) {
                        Ok(result) => {
                            metadata = Some(result);
                            error_token = None;
                        }
                        Err(e) => {
                            tracing::warn!("Error deserializing metadata from cache: {e}");
                            let response = self
                                .get_fungible_token_metadata(chain_id, contrat_address)
                                .await?;
                            metadata = response.data;
                            error_token = response.error;
                        }
                    };
                }
                Err(e) => {
                    match e {
                        CacheRepositoryError::Unknown(e) => {
                            tracing::warn!("Error getting metadata from cache: {e}")
                        }
                        CacheRepositoryError::KeyNotFound(e) => {
                            tracing::debug!("Error getting metadata from cache: {e}")
                        }
                    }
                    let response = self
                        .get_fungible_token_metadata(chain_id, contrat_address)
                        .await?;
                    metadata = response.data.clone();
                    error_token = response.error;
                }
            }

            // if there is metadata we store it in the data array
            if let Some(metadata) = metadata {
                retval.data.push(FungibleTokenInfoDetail {
                    contract_address: contrat_address,
                    balance: token.token_balance.to_string(),
                    decimals: metadata.decimals,
                    logo: metadata.logo,
                    name: metadata.name,
                    symbol: metadata.symbol,
                })
            }
            // If there was an error during the process of getting the metadata we store it in the errors array.
            if let Some(error) = error_token {
                retval.errors.push(error)
            }
        }

        Ok(retval)
    }

    async fn get_fungible_token_metadata(
        &self,
        chain_id: u64,
        address: Address,
    ) -> Result<FungibleTokenMetadataInfo> {
        let endpoint = self.get_evm_endpoint(chain_id, None).await?;
        let body = json!(
        {
            "jsonrpc" : "2.0",
            "id": chain_id.to_string(),
            "method" : "alchemy_getTokenMetadata",
            "params" : [format!("{:?}",address.clone())],
        });

        let response = self
            .http_client
            .post(endpoint.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context(format!(
                    "Error querying FT Metadata for contract {:?}",
                    address
                )))
            })?;

        let response_status = response.status();
        let response_body = response.text().await.map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Error obtaining http response"))
        })?;

        if !response_status.is_success() {
            return Err(BlockchainProviderError::Unknown(anyhow!(
                "Alchemy FT Metadata failed with status {}. Response: {}",
                response_status,
                response_body
            )));
        }

        match serde_json::from_str::<AlchemyGetMetadataResponse>(&response_body) {
            Ok(res) => {
                let key = format!("CONTRACT_ADDRESS#{}#CHAIN_ID#{}", address.clone(), chain_id);
                let data = Some(FungibleTokenMetadata {
                    decimals: res.result.decimals.unwrap_or(0).to_string(),
                    logo: res.result.logo.unwrap_or("".to_string()),
                    name: res.result.name,
                    symbol: res.result.symbol,
                });

                // save data to the cache.
                self.save_data_to_cache(data.clone(), DataType::FtMetadata, key.clone())
                    .await;
                Ok(FungibleTokenMetadataInfo { data, error: None })
            }
            Err(_) => {
                match serde_json::from_str::<AlchemyGetMetadataErrorResponse>(&response_body) {
                    Ok(error_respose) => {
                        Ok(FungibleTokenMetadataInfo {
                            data: None,
                            error: Some(TokenError {
                                contract_address: address,
                                reason: format!("There was an issue getting metadata: {:?}", error_respose.error.message),
                            }),
                        })
                    },
                    Err(erno) => {
                        return Err(BlockchainProviderError::Unknown(anyhow!(erno).context(format!(
                                                "Error deserializing Alchemy FT metadata endpoint response for contract: {:?}. Response Body: {:?}",
                                                address, response_body
                                            ))))
                    },
                }
            }
        }
    }

    async fn get_fee_history<'percentiles>(
        &self,
        chain_id: u64,
        block_count: u64,
        newest_block: NewestBlock,
        reward_percentiles: &'percentiles [f64],
    ) -> Result<FeeHistory> {
        let endpoint = self.get_evm_endpoint(chain_id, None).await?;

        let newest_block = if let NewestBlock::BlockNumber(n) = newest_block {
            Value::from(n)
        } else {
            Value::from("latest")
        };

        let body = json!({
            "jsonrpc":"2.0",
            "method":"eth_feeHistory",
            "params":[block_count, newest_block, reward_percentiles],
            "id": chain_id.to_string(),
        })
        .to_string();

        let response = self
            .http_client
            .post(endpoint)
            .body(body)
            .send()
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Error getting historical fees"),
                )
            })?
            .json::<AlchemyRPCResponse<FeeHistory>>()
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Error obtaining http response"),
                )
            })?;

        Ok(response.into_inner())
    }

    async fn tx_status_succeed(&self, chain_id: u64, tx_hash: String) -> Result<bool> {
        let provider = self.get_provider_for(chain_id).await?;

        let tx_hash = H256::from_str(tx_hash.as_ref()).map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Unable to get transaction hash"))
        })?;

        let receipt: Option<ethers::types::TransactionReceipt> = provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Unable to get transaction receipt"),
                )
            })?;

        let status = receipt.and_then(|receipt| receipt.status).ok_or_else(|| {
            BlockchainProviderError::Unknown(anyhow!("Receipt status not available"))
        })?;

        Ok(status == 1.into())
    }

    async fn get_tx_receipt(
        &self,
        chain_id: u64,
        tx_hash: String,
    ) -> Result<Option<ethers::types::TransactionReceipt>> {
        let provider = self.get_provider_for(chain_id).await?;

        let tx_hash = H256::from_str(tx_hash.as_ref()).map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Unable to get transaction hash"))
        })?;

        let receipt: Option<ethers::types::TransactionReceipt> = provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Unable to get transaction receipt"),
                )
            })?;

        Ok(receipt)
    }

    async fn get_tx_by_hash(&self, chain_id: u64, tx_hash: String) -> Result<Option<Transaction>> {
        let provider = self.get_provider_for(chain_id).await?;
        let tx_hash = H256::from_str(&tx_hash).map_err(|e| {
            BlockchainProviderError::Unknown(anyhow!(e).context("Unable to get transaction hash"))
        })?;
        let transaction: Option<Transaction> =
            provider.get_transaction(tx_hash).await.map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context("Unable to get transaction"))
            })?;

        Ok(transaction)
    }

    async fn get_current_nonce(&self, chain_id: u64, address: Address) -> Result<U256> {
        let provider = self.get_provider_for(chain_id).await?;
        let nonce = provider
            .get_transaction_count(address, None)
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(anyhow!(e).context("Unable to get transaction"))
            })?;

        Ok(nonce)
    }

    async fn get_fees_from_pending(&self, chain_id: u64) -> Result<BlockFeeQuery> {
        let provider = self.get_provider_for(chain_id).await?;

        let block = provider
            .get_block_with_txs(BlockNumber::Pending)
            .await
            .map_err(|e| {
                BlockchainProviderError::Unknown(
                    anyhow!(e).context("Unable to get transactions from pending block".to_string()),
                )
            })?
            .ok_or_else(|| {
                BlockchainProviderError::Unknown(anyhow!("Unable to get pending block"))
            })?;

        match block.base_fee_per_gas {
            Some(base_fee) => {
                let txs = block.transactions;

                let mut max_priority_gas_prices: Vec<U256> = txs
                    .iter()
                    .filter_map(|tx| tx.max_priority_fee_per_gas)
                    .collect();

                max_priority_gas_prices.sort_unstable();

                Ok(BlockFeeQuery {
                    max_priority_fees: max_priority_gas_prices,
                    base_fee_per_gas: base_fee,
                })
            }
            None => Err(BlockchainProviderError::Unknown(anyhow!(
                "Pending block not found",
            ))),
        }
    }
}
