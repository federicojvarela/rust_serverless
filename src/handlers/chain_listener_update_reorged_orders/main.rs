mod config;
mod dtos;

use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::secrets_manager::get_secrets_provider;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::LambdaError,
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;
use std::str::FromStr;
use std::sync::Arc;

use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::EvmBlockchainProvider;

use crate::dtos::UpdateOrdersStatusEvent;
use async_trait::async_trait;
use model::order::OrderState;

pub const TRANSACTION_HASH_INDEX_NAME: &str = "transaction_hash_index";

type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;

pub struct Persisted {
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub config: Config,
    pub blockchain_provider: BlockchainProviderObject,
}

pub struct MpcChainListenerUpdateOrder;

#[async_trait]
impl Lambda for MpcChainListenerUpdateOrder {
    type PersistedMemory = Persisted;
    type InputBody = UpdateOrdersStatusEvent;
    type Output = ();
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamo_db_client = get_dynamodb_client();

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let secrets_provider = get_secrets_provider().await;

        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));

        let blockchain_provider = Arc::new(AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        )) as BlockchainProviderObject;

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            dynamo_db_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            orders_repository,
            config,
            blockchain_provider,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let new_order_state = match OrderState::from_str(&request.detail.new_state) {
            Ok(order_state) => order_state,
            Err(e) => {
                let error = format!("Unknown event type {}", e);
                tracing::error!(error);
                return Err(LambdaError::Unknown(anyhow!(error)));
            }
        };

        let orders = state
            .orders_repository
            .get_orders_by_transaction_hashes(request.detail.hashes)
            .await
            .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

        for order in orders {
            let update_response = state
                .orders_repository
                .update_order_status(order.order_id.to_string(), new_order_state)
                .await;

            match update_response {
                Ok(_) => {
                    tracing::info!(
                        order_id = ?order.order_id,
                        "Order {} updated to state {}",
                        order.order_id,
                        order.state,
                    );
                }
                Err(error) => {
                    tracing::error!(
                        error = ?error,
                        order_id = ?order.order_id,
                        "Could not update Order {}. Current state {} \n {}",
                        order.order_id,
                        order.state,
                        error
                    );
                    continue;
                }
            }
        }

        Ok(())
    }
}
lambda_main!(MpcChainListenerUpdateOrder);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dtos::{Detail, UpdateOrdersStatusEvent};
    use crate::{MpcChainListenerUpdateOrder, Persisted};
    use async_trait::async_trait;
    use ethers::types::{Address, Transaction, U256};
    use mockall::{mock, predicate::eq};
    use model::order::OrderState;
    use mpc_signature_sm::blockchain::providers::*;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::LambdaError;
    use repositories::orders::*;
    use rstest::*;
    use std::sync::Arc;

    mock! {
        BlockchainProvider {}
        #[async_trait]
        impl EvmBlockchainProvider  for  BlockchainProvider {
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
        pub orders_repo: MockOrdersRepository,
        pub config: Config,
        pub request: UpdateOrdersStatusEvent,
        pub blockchain_provider: MockBlockchainProvider,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            orders_repo: MockOrdersRepository::new(),
            config: Config {
                order_status_table_name: "order_status".to_owned(),
                keys_table_name: "keys".to_owned(),
                cache_table_name: "cache".to_owned(),
            },
            request: UpdateOrdersStatusEvent {
                detail: Detail {
                    chain_id: 1,
                    new_state: OrderState::Reorged.to_string(),
                    hashes: vec!["0x123".to_owned(), "0x123".to_owned(), "0x123".to_owned()],
                },
            },
            blockchain_provider: MockBlockchainProvider::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn update_orders_invalid_state(mut fixture: TestFixture) {
        fixture.request.detail.new_state = "not_suportted_state".to_string();
        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                config: fixture.config,
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert_eq!(
            "Unknown event type Not supported OrderState variant: NOT_SUPORTTED_STATE",
            error.to_string()
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_orders_with_completed_state(mut fixture: TestFixture) {
        let mut transaction_hashes: Vec<String> = vec![];
        fixture.request.detail.new_state = "COMPLETED".to_string();
        fixture
            .request
            .detail
            .clone()
            .hashes
            .into_iter()
            .for_each(|hash| {
                transaction_hashes.push(hash);
            });

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hashes()
            .with(eq(transaction_hashes))
            .once()
            .returning(|_| Ok(vec![]));

        MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                config: fixture.config,
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .expect("should succeed");
    }
}
