use crate::impl_unknown_error_trait;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use model::order::{OrderState, OrderStatus, OrderSummary, OrderType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::orders::input_builder::UpdateOrderStatement;
#[cfg(feature = "test_mocks")]
use mockall::mock;

pub mod input_builder;
pub mod orders_repository_impl;

#[derive(Debug, thiserror::Error)]
pub enum OrdersRepositoryError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
    #[error("{0}")]
    OrderNotFound(String),
    #[error("conditional check failed: {0}")]
    ConditionalCheckFailed(String),
    #[error("tried to change order state without checking the previous state is correct")]
    PreviousStatesNotFound,
}

impl_unknown_error_trait!(OrdersRepositoryError);

const STATE_LAST_MODIFIED_AT_INDEX_NAME: &str = "last_modified_at_index";
const TRANSACTION_HASH_INDEX_NAME: &str = "transaction_hash_index";
const KEY_CHAIN_TYPE_INDEX_NAME: &str = "key_chain_type_index";

#[derive(Serialize)]
struct UpdateState {
    #[serde(rename(serialize = ":state"))]
    pub state: OrderState,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct UpdateStateAndLastModifiedTime {
    #[serde(rename(serialize = ":state"))]
    pub state: OrderState,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct UpdateBlockNumberAndHash {
    #[serde(rename(serialize = ":block_number"))]
    pub block_number: u64,
    #[serde(rename(serialize = ":block_hash"))]
    pub block_hash: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Order {
    pub order_id: Uuid,
    pub state: OrderState,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct UpdateReplacedBy {
    #[serde(rename(serialize = ":replaced_by"))]
    pub replaced_by: String,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct TransactionHashGSI {
    #[serde(rename(serialize = ":hash"))]
    pub hash: String,
}

#[derive(Serialize)]
struct QueryByStatusAndLastModified {
    #[serde(rename(serialize = ":current_state"))]
    pub state: String,
    #[serde(rename(serialize = ":last_modified_at"))]
    pub last_modified_at: String,
}

#[derive(Serialize)]
struct KeyChainTypeGSI {
    #[serde(rename(serialize = ":key_chain_type"))]
    pub key_chain_type: String,
    #[serde(rename(serialize = ":state"))]
    pub state: String,
}

impl From<anyhow::Error> for OrdersRepositoryError {
    fn from(error: anyhow::Error) -> Self {
        OrdersRepositoryError::Unknown(error)
    }
}

#[async_trait]
pub trait OrdersRepository
where
    Self: Sync + Send,
{
    async fn get_order_by_id(&self, order_id: String)
        -> Result<OrderStatus, OrdersRepositoryError>;

    async fn get_orders_by_transaction_hash(
        &self,
        transaction_hash: String,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

    async fn get_orders_by_transaction_hashes(
        &self,
        transaction_hash: Vec<String>,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

    // TODO: https://forteio.atlassian.net/browse/WALL-1534
    async fn update_order_status(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<(), OrdersRepositoryError>;

    async fn update_order_state_and_unlock_address(
        &self,
        cache_table_name: String,
        order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError>;

    // TODO: https://forteio.atlassian.net/browse/WALL-1534
    async fn update_order_status_and_execution_id_non_terminal_state(
        &self,
        order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError>;

    async fn update_order_state_with_replacement_and_unlock_address(
        &self,
        cache_table_name: String,
        order_id: String,
        original_order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError>;

    // TODO: https://forteio.atlassian.net/browse/WALL-1534
    async fn update_order_status_with_replacement_and_execution_id_non_terminal_state(
        &self,
        order_id: String,
        original_order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError>;

    // TODO: https://forteio.atlassian.net/browse/WALL-1534
    async fn update_order_status_and_tx_monitor_last_update(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<(), OrdersRepositoryError>;

    async fn update_order_status_block(
        &self,
        cache_table_name: String,
        order_id: String,
        new_state: OrderState,
        block_number: u64,
        block_hash: String,
    ) -> Result<(), OrdersRepositoryError>;

    /// Updates an order status and its replacement (or replaced).
    ///
    /// Sets the block number, block hash and new state to the mined order
    /// Sets a new state to the replaced (or replacement) order
    #[allow(clippy::too_many_arguments)]
    async fn update_order_and_replacement_with_status_block(
        &self,
        cache_table_name: String,
        mined_order_id: String,
        replaced_order_id: String,
        mined_new_state: OrderState,
        replaced_new_state: OrderState,
        block_number: u64,
        block_hash: String,
        locking_order_id: String,
        replaced_by_order_id: Option<String>,
    ) -> Result<(), OrdersRepositoryError>;

    async fn get_orders_by_status(
        &self,
        state: OrderState,
        last_modified_threshold: i64,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

    async fn request_cancellation(&self, order_id: String) -> Result<(), OrdersRepositoryError>;

    async fn create_replacement_order(
        &self,
        new_order: &OrderStatus,
    ) -> Result<(), OrdersRepositoryError>;

    async fn create_order(&self, order: &OrderStatus) -> Result<(), OrdersRepositoryError>;

    async fn get_orders_by_key_chain_type_state(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

    async fn get_orders_summary_by_key_chain_type_state(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
    ) -> Result<Vec<OrderSummary>, OrdersRepositoryError>;
}

#[cfg(feature = "test_mocks")]
mock! {
    pub OrdersRepository {}
    #[async_trait]
    impl OrdersRepository for OrdersRepository {
        async fn get_order_by_id(
            &self,
            order_id: String,
        ) -> Result<OrderStatus, OrdersRepositoryError>;

        async fn get_orders_by_transaction_hash(
            &self,
            transaction_hash: String,
        ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

        async fn get_orders_by_transaction_hashes(
            &self,
            transaction_hash: Vec<String>,
        ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

        async fn update_order_status(
            &self,
            order_id: String,
            new_state: OrderState,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_state_and_unlock_address(
            &self,
            cache_table_name: String,
            order_id: String,
            new_state: OrderState,
            update_order_statement: Option<UpdateOrderStatement>,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_status_and_execution_id_non_terminal_state(
            &self,
            order_id: String,
            new_state: OrderState,
            update_order_statement: Option<UpdateOrderStatement>,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_state_with_replacement_and_unlock_address(
            &self,
            cache_table_name: String,
            order_id: String,
            original_order_id: String,
            new_state: OrderState,
            update_order_statement: Option<UpdateOrderStatement>,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_status_with_replacement_and_execution_id_non_terminal_state(
            &self,
            order_id: String,
            original_order_id: String,
            new_state: OrderState,
            update_order_statement: Option<UpdateOrderStatement>,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_status_and_tx_monitor_last_update(
            &self,
            order_id: String,
            new_state: OrderState,
        ) -> Result<(), OrdersRepositoryError>;

        async fn update_order_status_block(
            &self,
            cache_table_name: String,
            order_id: String,
            new_state: OrderState,
            block_number: u64,
            block_hash: String,
        ) -> Result<(), OrdersRepositoryError>;

        #[allow(clippy::too_many_arguments)]
        async fn update_order_and_replacement_with_status_block(
            &self,
            cache_table_name: String,
            mined_order_id: String,
            replaced_order_id: String,
            mined_new_state: OrderState,
            replaced_new_state: OrderState,
            block_number: u64,
            block_hash: String,
            locking_order_id: String,
            replaced_by_order_id: Option<String>
        ) -> Result<(), OrdersRepositoryError>;

        async fn get_orders_by_status(
            &self,
            state: OrderState,
            last_modified_threshold: i64,
        ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

        async fn request_cancellation(&self, order_id: String) -> Result<(), OrdersRepositoryError>;

        async fn create_replacement_order(
            &self,
            new_order: &OrderStatus,
        ) -> Result<(), OrdersRepositoryError>;

        async fn create_order(&self, order: &OrderStatus) -> Result<(), OrdersRepositoryError>;

        async fn get_orders_by_key_chain_type_state(
            &self,
            key_id: String,
            chain_id: u64,
            order_type: OrderType,
            state: OrderState,
            limit: Option<i64>,
        ) -> Result<Vec<OrderStatus>, OrdersRepositoryError>;

        async fn get_orders_summary_by_key_chain_type_state(
            &self,
            key_id: String,
            chain_id: u64,
            order_type: OrderType,
            state: OrderState,
            limit: Option<i64>,
        ) -> Result<Vec<OrderSummary>, OrdersRepositoryError>;
    }
}
