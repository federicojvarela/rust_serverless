pub mod config;
pub mod event_bridge;

use self::event_bridge::EventBridgeError;
use async_trait::async_trait;
use ethers::types::Transaction;
pub use event_bridge::EventBridgePublisher;
use model::order::OrderState;
use uuid::Uuid;

#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait EventPublisher {
    async fn publish_dropped_order_event(&self, order_id: Uuid) -> Result<(), EventBridgeError>;

    async fn publish_admin_force_order_event(
        &self,
        order_id: Uuid,
        environment: String,
    ) -> Result<(), EventBridgeError>;

    async fn publish_stale_order_found_event(
        &self,
        order_id: Uuid,
        order_state: OrderState,
        environment: String,
    ) -> Result<(), EventBridgeError>;

    async fn publish_transaction_event(
        &self,
        transaction_details: Transaction,
        chain_id: u64,
        order_id: Uuid,
    ) -> Result<(), EventBridgeError>;
}
