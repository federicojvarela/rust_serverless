use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use ethers::types::Transaction;
use rusoto_dynamodb::DynamoDb;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::event_bridge::get_event_bridge_client;
use common::aws_clients::secrets_manager::get_secrets_provider;
use model::order::{OrderState, OrderStatus, OrderType};
use mpc_signature_sm::blockchain::providers::{
    alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider, EvmBlockchainProvider,
};
use mpc_signature_sm::publish::config::EbConfig;
use mpc_signature_sm::publish::{EventBridgePublisher, EventPublisher};
use mpc_signature_sm::{
    lambda_main,
    lambda_structure::lambda_trait::Lambda,
    result::error::LambdaError,
    validations::address::{address_validator::AddressValidatorImpl, AddressValidator},
};
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::orders::{orders_repository_impl::OrdersRepositoryImpl, OrdersRepository};

use crate::config::Config;
use crate::dtos::TransactionMonitorRequestEvent;

mod config;
mod dtos;

type DynamoDBClientObject = Arc<dyn DynamoDb + Sync + Send>;
type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;
type AddressValidatorObject = Arc<dyn AddressValidator + Sync + Send>;
type EventPublisherObject = Arc<dyn EventPublisher + Sync + Send>;

pub struct Persisted {
    pub dynamodb_client: DynamoDBClientObject,
    pub config: Config,
    pub eb_config: EbConfig,
    pub address_validator: AddressValidatorObject,
    pub blockchain_provider: BlockchainProviderObject,
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub order_age_threshold: Duration,
    pub event_publisher: EventPublisherObject,
}

pub struct MpcTxMonitor;

#[async_trait]
impl Lambda for MpcTxMonitor {
    type PersistedMemory = Persisted;
    type InputBody = TransactionMonitorRequestEvent;
    type Output = ();
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let eb_config = ConfigLoader::load_default::<EbConfig>();
        let dynamo_db_client = get_dynamodb_client();
        let address_validator = Arc::new(AddressValidatorImpl::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client.clone(),
        ))) as AddressValidatorObject;
        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));
        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            dynamo_db_client.clone(),
        )) as Arc<dyn OrdersRepository>;
        let order_age_threshold = config.order_age_threshold_in_secs;
        let dynamodb_client = Arc::new(dynamo_db_client) as DynamoDBClientObject;

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let secrets_provider = get_secrets_provider().await;

        let blockchain_provider = Arc::new(AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        )) as BlockchainProviderObject;

        let event_publisher = Arc::new(EventBridgePublisher::new(
            &eb_config,
            get_event_bridge_client(),
        )) as EventPublisherObject;

        Ok(Persisted {
            dynamodb_client,
            config,
            eb_config,
            address_validator,
            blockchain_provider,
            orders_repository,
            order_age_threshold: Duration::seconds(order_age_threshold),
            event_publisher,
        })
    }

    async fn run(_: TransactionMonitorRequestEvent, state: &Persisted) -> Result<(), LambdaError> {
        tracing::info!("Transaction Monitor Started at {}", Utc::now());

        process_submitted_orders(
            state,
            &[
                OrderType::Signature,
                OrderType::SpeedUp,
                OrderType::Cancellation,
            ],
        )
        .await?;
        process_stale_orders(state, OrderState::Signed).await?;
        process_stale_orders(state, OrderState::SelectedForSigning).await
    }
}

async fn process_submitted_orders(
    state: &Persisted,
    order_types: &[OrderType],
) -> Result<(), LambdaError> {
    tracing::info!("Processing SUBMITTED orders");

    let orders = state
        .orders_repository
        .get_orders_by_status(OrderState::Submitted, state.config.last_modified_threshold)
        .await
        .map_err(|e| {
            LambdaError::Unknown(
                anyhow!(e).context("Failed to retrieve SUBMITTED orders from database"),
            )
        })?;

    for order in orders {
        if !order_types.contains(&order.order_type) {
            continue;
        }

        tracing::info!(
            order_id = ?order.order_id,
                        "Current order state for order id {:?} is {:?} last modified at {:?}",
            order.order_id,
            order.state,
            order.last_modified_at
        );

        let order_chain_id;
        if let Some(chain_id) = order.data.extract_and_convert_chain_id() {
            order_chain_id = chain_id;
            tracing::info!(
                order_id = ?order.order_id,
                chain_id =  chain_id,
                "Chain ID({:?}) selected for order({:?})",
                chain_id,
                order.order_id
            );
        } else {
            tracing::warn!(
                order_id = order.order_id.to_string(),
                "Could not find chain_id for order({:?})",
                order.order_id
            );
            return Ok(());
        }

        match &order.state {
            OrderState::Submitted => {
                tracing::info!(
                    order_id = ?order.order_id,
                    "Getting tx_hash for order({:?})",
                    order.order_id
                );

                let tx_hash = match order.clone().transaction_hash {
                    Some(hash) => hash,
                    None => {
                        tracing::warn!(order_id = order.order_id.to_string(),
                                "Skipped Validation - Fail to retrieve transaction hash from Order: {} last modified at {}",
                                order.order_id.clone(),
                                order.last_modified_at,
                            );
                        return Ok(());
                    }
                };

                tracing::info!(
                    tx_hash = tx_hash,
                    order_id = ?order.order_id,
                    "Got Transaction Hash({:?}) selected for order({:?})",
                    tx_hash,
                    order.order_id
                );

                let order_transaction_details = state
                    .blockchain_provider
                    .get_tx_by_hash(order_chain_id, tx_hash.clone())
                    .await?;

                match order_transaction_details {
                    Some(transaction_details) => {
                        if valid_transaction(&transaction_details) {
                            let _ = process_transaction_receipt(
                                state,
                                order_chain_id,
                                &order,
                                tx_hash.clone(),
                                transaction_details,
                                state.event_publisher.clone(),
                            )
                            .await
                            .map_err(|e| {
                                tracing::error!(
                                    order_id = ?order.order_id,
                                    "Failed to validate order({:?}) receipt {e:}",
                                    order.order_id,
                                )
                            });
                        } else {
                            tracing::info!(
                                order_id = ?order.order_id,
                                chain_id = order_chain_id,tx_hash,
                                "Transaction({:?}) in Chain ID({:?}) is still in mempool for order {:?}",
                                tx_hash,order_chain_id,
                                order.order_id
                            );

                            // TODO: create new function just to update tx_monitor_last_modified_at field in DB
                            let _ =
                                update_status(state, &order, order_chain_id, OrderState::Submitted)
                                    .await;
                        }
                    }
                    None => {
                        // Update the state to dropped
                        // Dynamodb Stream sends the proper event to event bus
                        tracing::info!(
                            order_id = ?order.order_id,
                            chain_id = order_chain_id,
                            "Updating order({:?}) in Chain ID({:?}) with dropped state",
                            order.order_id,
                            order_chain_id
                        );
                        let _ =
                            update_status(state, &order, order_chain_id, OrderState::Dropped).await;
                    }
                }
            }
            _other => {}
        };
    }
    Ok(())
}

async fn process_stale_orders(
    state: &Persisted,
    order_state: OrderState,
) -> Result<(), LambdaError> {
    tracing::info!("Processing {} orders", order_state);

    let orders = state
        .orders_repository
        .get_orders_by_status(order_state, state.config.last_modified_threshold)
        .await
        .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

    for order in orders {
        if is_stale_order(state, order.clone()).await {
            state
                .event_publisher
                .publish_stale_order_found_event(
                    order.order_id,
                    order_state,
                    state.config.environment.clone(),
                )
                .await
                .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

            tracing::info!(
                "Sent an event for a {:?} order with order_id {:?}",
                order_state,
                order.order_id
            );
        }
    }
    Ok(())
}

async fn is_stale_order(state: &Persisted, order: OrderStatus) -> bool {
    tracing::info!(
        order_id = ?order.order_id,
        "Current order id {:?} was last modified at {:?}",
        order.order_id,
        order.last_modified_at
    );
    let order_age = Utc::now() - order.last_modified_at;
    tracing::info!(
        order_id = order.order_id.to_string(),
        "Age for order id {} is {}",
        order.order_id,
        order_age,
    );
    order_age < state.order_age_threshold
}

fn valid_transaction(transaction: &Transaction) -> bool {
    transaction.block_number.is_some()
        && transaction.block_hash.is_some()
        && transaction.transaction_index.is_some()
}

async fn process_transaction_receipt(
    state: &Persisted,
    chain_id: u64,
    order: &OrderStatus,
    tx_hash: String,
    transaction_details: Transaction,
    event_publisher: Arc<dyn EventPublisher + Sync + Send>,
) -> Result<(), LambdaError> {
    tracing::info!(
        order_id = ?order.order_id,
        chain_id = chain_id,
        "Validating transaction receipt for order id {:?} in Chain ID({:?}) last modified at {}",
        order.order_id,
        chain_id,
        order.last_modified_at
    );

    let receipt = state
        .blockchain_provider
        .get_tx_receipt(chain_id, tx_hash)
        .await?;

    match receipt {
        Some(_) => {
            tracing::info!(order_id = ?order.order_id,
                chain_id = chain_id,
                "Found a receipt for the order({:?}) in Chain ID({:?}), sending event to update nonce and order status",
                order.order_id,
                chain_id
            );

            let _ = event_publisher
                .publish_transaction_event(transaction_details, chain_id, order.order_id)
                .await;
        }
        None => {
            tracing::info!(
                order_id = ?order.order_id,
                chain_id =  chain_id,
                "Could not found a receipt for the order({:?}) in Chain ID({:?}), updating order status to dropped",
                order.order_id,
                chain_id
            );

            // TODO update related order (speed up or sponsored)
            // We only update the state to dropped and the Dynamodb Stream trigger
            // will send the proper event to event bus
            let _ = update_status(state, order, chain_id, OrderState::Dropped).await;
        }
    };
    Ok(())
}

async fn update_status(
    state: &Persisted,
    order: &OrderStatus,
    chain_id: u64,
    new_state: OrderState,
) -> Result<(), LambdaError> {
    let update_response = state
        .orders_repository
        .update_order_status_and_tx_monitor_last_update(order.order_id.to_string(), new_state)
        .await;

    match update_response {
        Ok(_) => {
            tracing::info!(
                order_id = ?order.order_id,
                chain_id = chain_id,
                "An order was updated Order: {:?} in Chain ID({:?}) State: {:?}",
                order.order_id.clone(),
                chain_id,
                new_state.clone()
            );
            Ok(())
        }
        Err(e) => Err(LambdaError::Unknown(anyhow!(format!(
            "Could not update Order {} state in Chain ID({:?}). Current state {} {e:}",
            order.order_id, order.state, chain_id,
        )))),
    }
}

lambda_main!(MpcTxMonitor);

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::Duration;
    use ethers::types::{Address, Transaction, U256};
    use mockall::mock;
    use rstest::*;
    use rusoto_core::Region;
    use uuid::Uuid;

    use common::test_tools::http::constants::HASH_FOR_MOCK_REQUESTS;
    use common::test_tools::mocks::dynamodb_client::*;
    use model::order::helpers::{
        build_cancellation_order, build_signature_order, build_sponsored_order,
    };
    use model::order::OrderState;
    use mpc_signature_sm::blockchain::providers::*;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::publish::config::EbConfig;
    use mpc_signature_sm::publish::event_bridge::EventBridgeError;
    use mpc_signature_sm::publish::EventPublisher;
    use mpc_signature_sm::validations::address::AddressValidator;
    use mpc_signature_sm::validations::address::AddressValidatorError;
    use repositories::orders::*;

    use crate::config::Config;
    use crate::dtos::TransactionMonitorRequestEvent;
    use crate::{MpcTxMonitor, Persisted};

    mock! {
        EBPublisher {}
        #[async_trait]
        impl EventPublisher for EBPublisher {
            async fn publish_dropped_order_event(&self, order_id: Uuid) -> Result<(), EventBridgeError>;
            async fn publish_admin_force_order_event(&self, order_id: Uuid, environment: String) -> Result<(), EventBridgeError>;
            async fn publish_stale_order_found_event(&self, order_id: Uuid, order_state: OrderState, environment: String) -> Result<(), EventBridgeError>;
            async fn publish_transaction_event(&self,transaction_details: Transaction,chain_id: u64,order_id: Uuid) -> Result<(), EventBridgeError>;
        }
    }

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

    mock! {
        AddrValidator {}
        #[async_trait]
        impl AddressValidator for AddrValidator {
            async fn valid_from_address(&self, address: String) -> Result<bool, AddressValidatorError>;
        }
    }

    struct TestFixture {
        pub dynamo_db_client: MockDbClient,
        pub config: Config,
        pub eb_config: EbConfig,
        pub request: TransactionMonitorRequestEvent,
        pub address_validator: MockAddrValidator,
        pub blockchain_provider: MockBlockchainProvider,
        pub orders_repo: MockOrdersRepository,
        pub order_age_threshold: Duration,
        pub event_publisher: MockEBPublisher,
    }

    const LAST_MODIFIED_THRESHOLD: i64 = 30;
    const ORDER_AGE_THRESHOLD: i64 = 60;

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            dynamo_db_client: MockDbClient::new(),
            config: Config {
                order_status_table_name: "order_status".to_owned(),
                keys_table_name: "keys".to_owned(),
                last_modified_threshold: LAST_MODIFIED_THRESHOLD,
                order_age_threshold_in_secs: ORDER_AGE_THRESHOLD,
                environment: "test".to_owned(),
                cache_table_name: "cache".to_owned(),
            },
            eb_config: EbConfig {
                aws_region: Region::UsWest2,
                event_bridge_event_bus_name: "test_bus".to_owned(),
            },
            request: TransactionMonitorRequestEvent {},
            address_validator: MockAddrValidator::new(),
            blockchain_provider: MockBlockchainProvider::new(),
            orders_repo: MockOrdersRepository::new(),
            order_age_threshold: Duration::seconds(ORDER_AGE_THRESHOLD),
            event_publisher: MockEBPublisher::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn tx_monitor_with_no_orders(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::SelectedForSigning)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Signed)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Submitted)
            .once()
            .returning(|_, _| Ok(vec![]));

        MpcTxMonitor::run(
            fixture.request,
            &Persisted {
                dynamodb_client: Arc::new(fixture.dynamo_db_client),
                config: fixture.config,
                eb_config: fixture.eb_config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
                orders_repository: Arc::new(fixture.orders_repo),
                order_age_threshold: fixture.order_age_threshold,
                event_publisher: Arc::new(fixture.event_publisher),
            },
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn tx_monitor_with_submitted_state(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::SelectedForSigning)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Signed)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Submitted)
            .once()
            .returning(|_, _| {
                Ok(vec![
                    build_signature_order(
                        Uuid::new_v4(),
                        OrderState::Submitted,
                        Some(HASH_FOR_MOCK_REQUESTS.to_string()),
                    ),
                    build_cancellation_order(
                        Uuid::new_v4(),
                        OrderState::Submitted,
                        Uuid::new_v4(),
                        None,
                        Some(HASH_FOR_MOCK_REQUESTS.to_string()),
                    ),
                ])
            });

        fixture
            .blockchain_provider
            .expect_get_tx_by_hash()
            .times(2)
            .returning(|_, _| Ok(None));

        fixture
            .orders_repo
            .expect_update_order_status_and_tx_monitor_last_update()
            .times(2)
            .returning(|_, _| Ok(()));

        MpcTxMonitor::run(
            fixture.request,
            &Persisted {
                dynamodb_client: Arc::new(fixture.dynamo_db_client),
                config: fixture.config,
                eb_config: fixture.eb_config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
                orders_repository: Arc::new(fixture.orders_repo),
                order_age_threshold: fixture.order_age_threshold,
                event_publisher: Arc::new(fixture.event_publisher),
            },
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn tx_monitor_with_sponsored_state(mut fixture: TestFixture) {
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::SelectedForSigning)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Signed)
            .once()
            .returning(move |_, _| Ok(vec![]));
        fixture
            .orders_repo
            .expect_get_orders_by_status()
            .withf(move |withf_status, _| *withf_status == OrderState::Submitted)
            .once()
            .returning(|_, _| {
                Ok(vec![build_sponsored_order(
                    Uuid::new_v4(),
                    OrderState::Submitted,
                )])
            });

        fixture
            .blockchain_provider
            .expect_get_tx_by_hash()
            .times(0)
            .returning(|_, _| Ok(None));

        MpcTxMonitor::run(
            fixture.request,
            &Persisted {
                dynamodb_client: Arc::new(fixture.dynamo_db_client),
                config: fixture.config,
                eb_config: fixture.eb_config,
                address_validator: Arc::new(fixture.address_validator),
                blockchain_provider: Arc::new(fixture.blockchain_provider),
                orders_repository: Arc::new(fixture.orders_repo),
                order_age_threshold: fixture.order_age_threshold,
                event_publisher: Arc::new(fixture.event_publisher),
            },
        )
        .await
        .expect("should succeed");
    }
}
