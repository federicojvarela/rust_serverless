use crate::publish::config::EbConfig;
use crate::publish::EventPublisher;
use anyhow::anyhow;
use async_trait::async_trait;
use ethers::types::Transaction;
use eventbridge_connector::{EventBridge, EventBuilder};
use model::order::OrderState;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub const CHAIN_LISTENER_PREFIX: &str = "ana-chain-listener";

#[derive(Debug, thiserror::Error)]
pub enum EventBridgeError {
    #[error("{0:#}")]
    Unknown(#[source] anyhow::Error),
}

pub struct EventBridgePublisher<EB>
where
    EB: EventBridge,
{
    event_bridge_client: Arc<EB>,
    event_bus_name: String,
}

impl<EB> EventBridgePublisher<EB>
where
    EB: EventBridge,
{
    pub fn new(config: &EbConfig, event_bridge_client: EB) -> Self {
        Self {
            event_bridge_client: Arc::new(event_bridge_client),
            event_bus_name: config.event_bridge_event_bus_name.clone(),
        }
    }
}

#[async_trait]
impl<EB> EventPublisher for EventBridgePublisher<EB>
where
    EB: EventBridge,
{
    async fn publish_dropped_order_event(&self, order_id: Uuid) -> Result<(), EventBridgeError> {
        let source = format!("{}-dropped-order", &order_id);

        tracing::info!(
            order_id = ?order_id,
            "Beginning submission of dropped transaction with order_id {:?}",
            order_id
        );

        let event = EventBuilder::new(
            json!({ "order_id": order_id }),
            "publish_dropped_event",
            source,
        )
        .event_bus_name(&self.event_bus_name)
        .map_err(|e| {
            EventBridgeError::Unknown(
                anyhow::anyhow!(e).context("unable to build event bridge event"),
            )
        })?
        .build();

        let eb_response = self
            .event_bridge_client
            .clone()
            .put_events(vec![event])
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Request failed with error: {e:?}");
                EventBridgeError::Unknown(e.into())
            })?;

        tracing::info!(
            order_id = ?order_id,
            "Finished submitting dropped transaction to event bridge.\\n{:?}",
            eb_response
        );

        Ok(())
    }

    async fn publish_admin_force_order_event(
        &self,
        order_id: Uuid,
        environment: String,
    ) -> Result<(), EventBridgeError> {
        let source = format!("{}-admin-force-order-selection", environment);

        tracing::info!(
            order_id = ?order_id,
            "Beginning submission of select order for signing admin force with order_id\\n{:?}",
            order_id
        );

        let event = EventBuilder::new(json!({ "order_id": order_id }), "admin_force_event", source)
            .event_bus_name(&self.event_bus_name)
            .map_err(|e| {
                EventBridgeError::Unknown(
                    anyhow::anyhow!(e).context("unable to build event bridge event"),
                )
            })?
            .build();

        let eb_response = self
            .event_bridge_client
            .clone()
            .put_events(vec![event])
            .await
            .map_err(|e| EventBridgeError::Unknown(anyhow!(e)))?;

        tracing::info!(
            order_id = ?order_id,
            "Finished submitting select order for signing admin force to event bridge.\\n{:?}",
            eb_response
        );

        Ok(())
    }

    async fn publish_stale_order_found_event(
        &self,
        order_id: Uuid,
        order_state: OrderState,
        environment: String,
    ) -> Result<(), EventBridgeError> {
        let source = format!(
            "{}-found-stale-{}-order-event",
            environment,
            order_state.as_str().to_lowercase()
        );

        tracing::info!(
            order_id = ?order_id,
            "Beginning submission of a stale order with order_id to select order for signing \\n{:?}",
            order_id
        );

        let event = EventBuilder::new(json!({ "order_id": order_id }), "stale_order_check", source)
            .event_bus_name(&self.event_bus_name)
            .map_err(|e| {
                EventBridgeError::Unknown(
                    anyhow::anyhow!(e).context("unable to build event bridge event"),
                )
            })?
            .build();

        let eb_response = self
            .event_bridge_client
            .clone()
            .put_events(vec![event])
            .await
            .map_err(|e| EventBridgeError::Unknown(anyhow!(e)))?;

        tracing::info!(
            order_id = ?order_id,
            "Finished submitting stale order with order_id {:?} to event bridge.\\n{:?}",
            order_id, eb_response
        );

        Ok(())
    }

    async fn publish_transaction_event(
        &self,
        transaction_details: Transaction,
        chain_id: u64,
        order_id: Uuid,
    ) -> Result<(), EventBridgeError> {
        // TODO: refactor this function to pull chain data and remove the code bellow
        let chain_name = match chain_id {
            1 | 11155111 => "ethereum",
            137 | 80002 => "polygon",
            _ => "Unsuported chain",
        };

        tracing::info!(
            order_id = ?order_id,
            "Beginning submission of order {:?} to event bus",
            order_id
        );

        let chain_type = format!("{}-{}", chain_name, chain_id);
        let source = format!("{}-{}", CHAIN_LISTENER_PREFIX, chain_type);

        let event = EventBuilder::new(
            json!({
                "hash": transaction_details.hash,
                "nonce": transaction_details.nonce,
                "from": transaction_details.from,
                "chainId": format!("0x{:x}", chain_id),
                "blockNumber": transaction_details.block_number,
                "blockHash": transaction_details.block_hash
            }),
            "publish_tx_event_tx_monitor",
            source,
        )
        .event_bus_name(&self.event_bus_name)
        .map_err(|e| {
            EventBridgeError::Unknown(
                anyhow::anyhow!(e).context("unable to build event bridge event"),
            )
        })?
        .build();

        let eb_response = self
            .event_bridge_client
            .clone()
            .put_events(vec![event])
            .await
            .map_err(|e| tracing::error!(error = ?e, "Request failed with error: {e:?}"));

        tracing::info!(
            order_id = ?order_id,
            "Finished submitting transaction event to event bus. {:?}",
            eb_response
        );

        Ok(())
    }
}
