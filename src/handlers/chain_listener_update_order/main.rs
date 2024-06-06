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
use std::sync::Arc;

use mpc_signature_sm::blockchain::providers::{
    alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider, EvmBlockchainProvider,
};

use crate::dtos::{MpcUpdateOrderResponse, Order, TransactionIncludedInBlockEvent};
use async_trait::async_trait;
use model::order::{OrderState, OrderType};
use mpc_signature_sm::validations::address::address_validator::AddressValidatorImpl;
use mpc_signature_sm::validations::address::AddressValidator;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::{OrdersRepository, OrdersRepositoryError};

type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;
type AddressValidatorObject = Arc<dyn AddressValidator + Sync + Send>;

pub struct Persisted {
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub cache_table_name: String,
    pub config: Config,
    pub address_validator: AddressValidatorObject,
    pub blockchain_provider: BlockchainProviderObject,
}

pub const ORDER_NOT_FOUND: &str = "order_not_found";

pub struct MpcChainListenerUpdateOrder;

#[async_trait]
impl Lambda for MpcChainListenerUpdateOrder {
    type PersistedMemory = Persisted;
    type InputBody = TransactionIncludedInBlockEvent;
    type Output = MpcUpdateOrderResponse;
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamo_db_client = get_dynamodb_client();
        let address_validator = Arc::new(AddressValidatorImpl::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client.clone(),
        ))) as AddressValidatorObject;

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let secrets_provider = get_secrets_provider().await;

        let cache_table_name = config.cache_table_name.clone();
        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            cache_table_name.clone(),
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
            cache_table_name,
            config,
            address_validator,
            blockchain_provider,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let tx_hash = request.detail.hash.clone();

        tracing::info!(
            tx_hash = tx_hash,
            "processing tx hash {:?}",
            tx_hash.clone(),
        );

        // Check if the address should be handled
        let valid = state
            .address_validator
            .valid_from_address(request.detail.from.clone())
            .await?;

        if !valid {
            // if we do not have the address in our DB we return without error
            tracing::debug!(
                chain_id = ?request.detail.chain_id,
                tx_hash = tx_hash,
                "from address {:?} in tx {:?} not found",
                request.detail.from.clone(),
                tx_hash.clone(),
            );
            return Ok(MpcUpdateOrderResponse { order_id: None });
        }

        let chain_id = request.detail.chain_id;
        let block_number: u64 = request.detail.block_number;
        let block_hash: String = request.detail.block_hash.clone();
        let order_record = get_order_state(request, state).await?;

        tracing::info!(
            order_id = ?order_record.order_id,
            tx_hash = tx_hash,
            "tx hash {:?} was found in order id {:?} with state {:?}",
            tx_hash.clone(),
            order_record.order_id,
            order_record.state,
        );

        match order_record.state {
            // HAPPY_PATH: ideal outcome
            OrderState::Submitted => {
                let tx_status_succeed = state
                    .blockchain_provider
                    .tx_status_succeed(chain_id, tx_hash.clone())
                    .await?;

                tracing::info!(
                    order_id = ?order_record.order_id,
                    tx_hash = tx_hash,
                    "status for tx hash {:?} is {:?}",
                    tx_hash.clone(),
                    tx_status_succeed
                );

                let new_state = if tx_status_succeed {
                    tracing::info!(
                        order_id = ?order_record.order_id,
                        chain_id = chain_id,
                        "New order state for order id {:?} will be {:?} in Chain ID({:?})",
                        order_record.order_id,
                        OrderState::Completed,
                        chain_id
                    );
                    OrderState::Completed
                } else {
                    tracing::info!(
                        order_id = ?order_record.order_id,
                        chain_id = chain_id,
                        "New order state for order id {:?} will be {:?} in Chain ID({:?})",
                        order_record.order_id,
                        OrderState::CompletedWithError,
                        chain_id
                    );
                    OrderState::CompletedWithError
                };

                // If the "replaces" field is present, it means this is a replacement order.
                // so the original order needs to set as REPLACED if it's a speedup order
                // or the same state if not.
                if let Some(original_order_id) = order_record.replaces {
                    let original_order = state
                        .orders_repository
                        .get_order_by_id(original_order_id.to_string())
                        .await
                        .map_err(|e| LambdaError::Unknown(e.into()))?;

                    // Sponsored orders are updated by a specialized state machine, not here
                    if original_order.order_type != OrderType::Sponsored {
                        state
                            .orders_repository
                            .update_order_and_replacement_with_status_block(
                                state.cache_table_name.clone(),
                                order_record.order_id.to_string(),
                                original_order_id.to_string(),
                                new_state,
                                OrderState::Replaced,
                                block_number,
                                block_hash,
                                original_order_id.to_string(),
                                Some(order_record.order_id.to_string()),
                            )
                            .await
                            .map_err(|e| LambdaError::Unknown(e.into()))?;

                        return Ok(MpcUpdateOrderResponse {
                            order_id: Some(order_record.order_id),
                        });
                    }
                }

                /*
                1. If the order does not have a replacement and does not replace another,
                it means it's a standard signature order with no speedups, so it needs
                only a normal update.
                2. If the replaced order is of type SPONSORED, it means it's a wrapper order,
                so it also needs only a normal update. The replaced sponsored order is updated
                by another specialized state machine.
                 */
                state
                    .orders_repository
                    .update_order_status_block(
                        state.cache_table_name.clone(),
                        order_record.order_id.to_string(),
                        new_state,
                        block_number,
                        block_hash,
                    )
                    .await
                    .map_err(|e| LambdaError::Unknown(e.into()))?;

                Ok(MpcUpdateOrderResponse {
                    order_id: Some(order_record.order_id),
                })
            }

            // HAPPY_PATH: already completed (alternative good outcome)
            OrderState::Completed => Ok(MpcUpdateOrderResponse {
                order_id: Some(order_record.order_id),
            }),

            other => Err(LambdaError::Unknown(anyhow!(format!(
                "Order needs to be in {} state but is in {} state",
                OrderState::Submitted,
                other
            )))),
        }
    }
}

async fn get_order_state(
    request: TransactionIncludedInBlockEvent,
    state: &Persisted,
) -> Result<Order, LambdaError> {
    let hash = request.detail.hash;

    let orders = state
        .orders_repository
        .get_orders_by_transaction_hash(hash)
        .await
        .map_err(|e| match e {
            OrdersRepositoryError::Unknown(e) => LambdaError::Unknown(e),
            OrdersRepositoryError::ConditionalCheckFailed(e) => LambdaError::Unknown(anyhow!(e)),
            OrdersRepositoryError::PreviousStatesNotFound => LambdaError::Unknown(anyhow::anyhow!(
                "tried to transition state without checking previous state in code"
            )),
            OrdersRepositoryError::OrderNotFound(message) => LambdaError::Unknown(anyhow!(message)),
        })?;

    if orders.len() == 1 {
        let order = orders.first().unwrap();
        return Ok(Order {
            order_id: order.order_id,
            state: order.state,
            replaced_by: order.replaced_by,
            replaces: order.replaces,
        });
    }

    let mut submitted_orders: Vec<Order> = orders
        .into_iter()
        .filter(|o| o.state == OrderState::Submitted && o.order_type != OrderType::Sponsored)
        .map(|o| Order {
            order_id: o.order_id,
            state: o.state,
            replaced_by: o.replaced_by,
            replaces: o.replaces,
        })
        .collect();

    if submitted_orders.len() > 1 {
        return Err(LambdaError::Unknown(anyhow!(
            "More than one submitted transaction found."
        )));
    }

    let order_record = submitted_orders
        .pop()
        .ok_or(LambdaError::Unknown(anyhow!("Transaction hash not found.")))?;

    Ok(order_record)
}

lambda_main!(MpcChainListenerUpdateOrder);

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::anyhow;
    use async_trait::async_trait;
    use ethers::types::{Address, Transaction, U256};
    use mockall::{mock, predicate, predicate::eq};
    use rstest::*;
    use uuid::Uuid;

    use model::order::helpers::{build_signature_order, build_sponsored_order};
    use model::order::{OrderState, OrderStatus};
    use mpc_signature_sm::blockchain::providers::*;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::LambdaError;
    use mpc_signature_sm::validations::address::AddressValidator;
    use mpc_signature_sm::validations::address::AddressValidatorError;
    use repositories::orders::*;

    use crate::config::Config;
    use crate::dtos::{Detail, TransactionIncludedInBlockEvent};
    use crate::{MpcChainListenerUpdateOrder, Persisted};

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
        AddrValidator {}
        #[async_trait]
        impl AddressValidator for AddrValidator {
            async fn valid_from_address(&self, address: String) -> Result<bool, AddressValidatorError>;
        }
    }

    struct TestFixture {
        pub orders_repo: MockOrdersRepository,
        pub config: Config,
        pub request: TransactionIncludedInBlockEvent,
        pub address_validator: MockAddrValidator,
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
            request: TransactionIncludedInBlockEvent {
                detail: Detail {
                    hash: "0x123".to_owned(),
                    from: "0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0".to_owned(),
                    chain_id: 1,
                    block_number: 18279003,
                    block_hash:
                        "0x1fe241c2ad88ff41168aa4318614327489201e604bd5a43b602ec3d631615145"
                            .to_owned(),
                },
            },
            address_validator: MockAddrValidator::new(),
            blockchain_provider: MockBlockchainProvider::new(),
        }
    }

    fn create_signature_order_status(
        order_id: Uuid,
        state: OrderState,
        transaction_hash: Option<String>,
        replaced_by: Option<Uuid>,
        replaces: Option<Uuid>,
    ) -> OrderStatus {
        let mut order_status = build_signature_order(order_id, state, transaction_hash);
        order_status.replaced_by = replaced_by;
        order_status.replaces = replaces;
        order_status
    }

    fn create_sponsored_order_status(
        order_id: Uuid,
        state: OrderState,
        replaced_by: Option<Uuid>,
        replaces: Option<Uuid>,
    ) -> OrderStatus {
        let mut order_status = build_sponsored_order(order_id, state);
        order_status.replaced_by = replaced_by;
        order_status.replaces = replaces;
        order_status
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_invalid_state(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(fixture.request.detail.hash.clone()))
            .once()
            .returning(|_| {
                Ok(vec![build_signature_order(
                    Uuid::new_v4(),
                    OrderState::Signed, // not completed or submitted
                    None,
                )])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .never();

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert_eq!(
            "Order needs to be in SUBMITTED state but is in SIGNED state",
            error.to_string()
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_db_error(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(fixture.request.detail.hash.clone()))
            .once()
            .returning(|_| Err(OrdersRepositoryError::Unknown(anyhow!("timeout!"))));

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn updated_order_transaction_hash_not_found(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(fixture.request.detail.hash.clone()))
            .once()
            .returning(|_| {
                Err(OrdersRepositoryError::OrderNotFound(
                    "Transaction hash not found.".to_owned(),
                ))
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert_eq!("Transaction hash not found.", error.to_string());
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_duplicated_hash(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![
                    build_signature_order(
                        Uuid::new_v4(),
                        OrderState::Submitted,
                        Some(hash.clone()),
                    ),
                    build_signature_order(
                        Uuid::new_v4(),
                        OrderState::Submitted,
                        Some(hash.clone()),
                    ),
                ])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert_eq!(
            "More than one submitted transaction found.",
            error.to_string()
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_already_completed(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();

        let order_id = Uuid::new_v4();

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![build_signature_order(
                    order_id,
                    OrderState::Completed,
                    Some(hash.clone()),
                )])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .never();

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_fail_to_update(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let block_number = 18279003;
        let block_hash =
            "0x1fe241c2ad88ff41168aa4318614327489201e604bd5a43b602ec3d631615145".to_string();
        let hash = fixture.request.detail.hash.clone();
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![build_signature_order(
                    order_id,
                    OrderState::Submitted,
                    Some(hash.clone()),
                )])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .with(
                eq(fixture.config.cache_table_name.clone()),
                eq(order_id.to_string()),
                eq(OrderState::Completed),
                eq(block_number),
                eq(block_hash),
            )
            .once()
            .returning(|_, _, _, _, _| Err(OrdersRepositoryError::Unknown(anyhow!("timeout!"))));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_with_completed_state(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let block_number = 18279003;
        let block_hash =
            "0x1fe241c2ad88ff41168aa4318614327489201e604bd5a43b602ec3d631615145".to_string();
        let hash = fixture.request.detail.hash.clone();
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![build_signature_order(
                    order_id,
                    OrderState::Submitted,
                    Some(hash.clone()),
                )])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .with(
                eq(fixture.config.cache_table_name.clone()),
                eq(order_id.to_string()),
                eq(OrderState::Completed),
                eq(block_number),
                eq(block_hash.to_string()),
            )
            .once()
            .returning(|_, _, _, _, _| Ok(()));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_with_error_state(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let hash = fixture.request.detail.hash.clone();
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![build_signature_order(
                    order_id,
                    OrderState::Submitted,
                    Some(hash.clone()),
                )])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .once()
            .returning(|_, _, _, _, _| Ok(()));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(false));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_db_error_key_address(mut fixture: TestFixture) {
        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Err(AddressValidatorError::Unknown(anyhow!("timeout!"))));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn updated_order_key_address_not_found(mut fixture: TestFixture) {
        //this mock has a never() call to check the code is returning early
        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .never();

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(false));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, None);
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_duplicated_hash_one_submitted(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();
        let order_id = Uuid::new_v4();

        let mut sponsored_order = build_sponsored_order(order_id, OrderState::Submitted);
        sponsored_order.transaction_hash = Some(hash.clone());

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![
                    build_signature_order(Uuid::new_v4(), OrderState::Signed, Some(hash.clone())),
                    build_signature_order(order_id, OrderState::Submitted, Some(hash.clone())),
                    sponsored_order.clone(),
                ])
            });

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .once()
            .returning(|_, _, _, _, _| Ok(()));

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_and_its_related(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();
        let original = Uuid::new_v4();
        let replacement = Uuid::new_v4();

        let original_order = create_signature_order_status(
            original,
            OrderState::Signed,
            Some(hash.clone()),
            Some(replacement),
            None,
        );

        let cloned = original_order.clone();

        let replacement_order = create_signature_order_status(
            replacement,
            OrderState::Submitted,
            Some(hash.clone()),
            None,
            Some(original),
        );

        fixture
            .orders_repo
            .expect_get_order_by_id()
            .with(eq(original.to_string()))
            .once()
            .returning(move |_| Ok(cloned.clone()));

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| Ok(vec![original_order.clone(), replacement_order.clone()]));

        fixture
            .orders_repo
            .expect_update_order_and_replacement_with_status_block()
            .times(1)
            .returning(|_, _, _, _, _, _, _, _, _| Ok(()));

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(replacement));
    }

    #[rstest]
    #[tokio::test]
    async fn update_original_order_and_not_its_related(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();
        let original = Uuid::new_v4();
        let replacement = Uuid::new_v4();

        let replacement_order = create_signature_order_status(
            replacement,
            OrderState::Signed,
            Some(hash.clone()),
            None,
            Some(original),
        );

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| {
                Ok(vec![
                    create_signature_order_status(
                        original,
                        OrderState::Submitted,
                        Some(hash.clone()),
                        Some(replacement),
                        None,
                    ),
                    replacement_order.clone(),
                ])
            });

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let block_number = 18279003;
        let block_hash =
            "0x1fe241c2ad88ff41168aa4318614327489201e604bd5a43b602ec3d631615145".to_string();

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .with(
                eq(fixture.config.cache_table_name.clone()),
                eq(original.to_string()),
                eq(OrderState::Completed),
                eq(block_number),
                eq(block_hash.to_string()),
            )
            .once()
            .returning(|_, _, _, _, _| Ok(()));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(original));
    }

    #[rstest]
    #[tokio::test]
    async fn skip_update_sponsored_order(mut fixture: TestFixture) {
        let hash = fixture.request.detail.hash.clone();
        let original = Uuid::new_v4();
        let replacement = Uuid::new_v4();

        let original_order =
            create_sponsored_order_status(original, OrderState::Signed, Some(replacement), None);

        let cloned = original_order.clone();

        let replacement_order = create_signature_order_status(
            replacement,
            OrderState::Submitted,
            Some(hash.clone()),
            None,
            Some(original),
        );

        fixture
            .orders_repo
            .expect_get_order_by_id()
            .with(eq(original.to_string()))
            .once()
            .returning(move |_| Ok(cloned.clone()));

        fixture
            .orders_repo
            .expect_get_orders_by_transaction_hash()
            .with(eq(hash.clone()))
            .once()
            .returning(move |_| Ok(vec![original_order.clone(), replacement_order.clone()]));

        fixture
            .orders_repo
            .expect_update_order_status_block()
            .once()
            .returning(|_, _, _, _, _| Ok(()));

        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(fixture.request.detail.from.clone()))
            .returning(|_| Ok(true));

        fixture
            .blockchain_provider
            .expect_tx_status_succeed()
            .once()
            .returning(|_, _| Ok(true));

        let result = MpcChainListenerUpdateOrder::run(
            fixture.request,
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repo),
                cache_table_name: fixture.config.cache_table_name.clone(),
                config: fixture.config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.order_id, Some(replacement));
    }
}
