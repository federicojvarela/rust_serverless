use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;

use common::aws_clients::dynamodb::get_dynamodb_client;
use model::order::{OrderState, OrderType};
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::Result,
};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;
use uuid::Uuid;

use crate::config::Config;
use crate::dtos::MpcUpdateOrderStatusRequest;

mod config;
mod dtos;

pub struct Persisted {
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub cache_table_name: String,
}

pub struct OrderStatusUpdater;

fn report_and_return_error<E: Into<anyhow::Error> + std::fmt::Debug>(
    error: E,
    order_id: Uuid,
) -> OrchestrationError {
    tracing::error!(error = ?error, order_id = ?order_id, "Request to update order_id {order_id} failed with error: {error:?}");
    OrchestrationError::from(anyhow!(error))
}

#[async_trait]
impl Lambda for OrderStatusUpdater {
    type PersistedMemory = Persisted;
    type InputBody = Event<MpcUpdateOrderStatusRequest>;
    type Output = ();
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name,
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            orders_repository,
            cache_table_name: config.cache_table_name,
        })
    }

    async fn run(request: Self::InputBody, state: &Self::PersistedMemory) -> Result<Self::Output> {
        // NOTE: order repository methods names are misleading.
        // update_order_status_with_replacement_and_execution_id_non_terminal_state and
        // update_order_status_and_execution_id_non_terminal_state
        // have nothing to do with execution_id and terminal/non terminal states, they just
        // transition the state checking that the state you are coming from is invalid and without
        // removing any lock
        // https://forteio.atlassian.net/browse/WALL-1534
        // This values are just for info
        let order_id = request.payload.order_id;
        let next_state = request.payload.next_state;
        tracing::info!(order_id = ?order_id, "Calling mpc_update_order {:?}", &request.payload);

        let order = state
            .orders_repository
            .get_order_by_id(request.payload.order_id.to_string())
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e)))?;

        // Fetches the replaces order if it exist
        let replaces_order = match order.replaces {
            Some(replaces_id) => Some(
                state
                    .orders_repository
                    .get_order_by_id(replaces_id.to_string())
                    .await
                    .map_err(|e| OrchestrationError::from(anyhow!(e)))?,
            ),
            None => None,
        };

        match (order.order_type, request.payload.next_state, replaces_order) {
            //
            // Signature orders are in charge of locking and unlocking the transactions.
            //

            // This is a special case for Sponsored transactions
            // We need to update the SUBMITTED state to both orders (Sponsored and Wrapped)
            (OrderType::Signature, OrderState::Submitted, Some(replaces_order))
                if replaces_order.state == OrderState::Signed
                    && replaces_order.order_type == OrderType::Sponsored =>
            {
                state
                    .orders_repository
                    .update_order_status_with_replacement_and_execution_id_non_terminal_state(
                        request.payload.order_id.to_string(),
                        replaces_order.order_id.to_string(),
                        request.payload.next_state,
                        request.payload.update_order_statement,
                    )
                    .await
                    .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
            }

            // Sponsored Transaction Error
            (OrderType::Signature, OrderState::Error, Some(replaces_order))
                if replaces_order.state == OrderState::Signed
                    && replaces_order.order_type == OrderType::Sponsored =>
            {
                process_error_next_state_with_replacement(
                    state,
                    request,
                    replaces_order.order_id.to_string(),
                )
                .await?;
            }

            // The error case is special, depends a lot on what state the order is coming from
            (OrderType::Signature, OrderState::Error, _) => {
                process_error_next_state(state, request).await?;
            }

            (OrderType::Signature, next_state, _) => {
                // If a Signature order transitions to a pending state, then we do NOT need to remove
                // the lock
                if next_state.is_pending_state() {
                    state
                        .orders_repository
                        .update_order_status_and_execution_id_non_terminal_state(
                            request.payload.order_id.to_string(),
                            request.payload.next_state,
                            request.payload.update_order_statement,
                        )
                        .await
                        .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
                    if order.state == OrderState::Submitted
                        && order.cancellation_requested.is_some()
                    {
                        tracing::info!(submitted_cancellation_requested = ?true, "Order Submitted with Cancellation requested true");
                    }
                }
                // If a Signature order transitions to a terminal state, then we do need to remove
                // the lock
                else {
                    state
                        .orders_repository
                        .update_order_state_and_unlock_address(
                            state.cache_table_name.clone(),
                            request.payload.order_id.to_string(),
                            request.payload.next_state,
                            request.payload.update_order_statement,
                        )
                        .await
                        .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
                }
            }

            // If it is an Sponsored, SpeedUp or a Cancellation we do not need to do anything with
            // address locks, so we just transition the state
            (OrderType::Sponsored, _, _)
            | (OrderType::SpeedUp, _, _)
            | (OrderType::Cancellation, _, _) => state
                .orders_repository
                .update_order_status_and_execution_id_non_terminal_state(
                    request.payload.order_id.to_string(),
                    request.payload.next_state,
                    request.payload.update_order_statement,
                )
                .await
                .map_err(|e| report_and_return_error(e, request.payload.order_id))?,

            // KeyCreation order should never be processed by this lambda
            (OrderType::KeyCreation, _, _) => {
                return Err(OrchestrationError::unknown(format!(
                    "found KeyCreation order with id {}",
                    order.order_id
                )));
            }
        }

        tracing::info!(
            order_id = ?order_id,
            "Order_id {:?} was marked as {:?}",
            order_id,
            next_state
        );

        Ok(())
    }
}

// Error state is an special case because the transiton can be done from practically any
// state "locking" or not. If it comes from a "locking" state (if an current state is
// "locking" means that the order's address is locked) and we transition to Error we need
// to remove the lock, otherwise we don't have to remove it.
//
// This is the only state where we also need the current order state to transition, because
// we can't use the transaction's ConditionCheck to check the previous order status (to
// know if we need to remove the lock or note checking the current state, because we can't
// use ConditionCheck and other operation on the same item in a trasnaction) and we can't
// rely on the idempotency of the Delete operation because the delete operation contains a
// conditional check.
//
// What happens if the operation fails? The lambda will fail and we will rely on the retry
// mechanisms of the State Machines
async fn process_error_next_state(
    state: &<OrderStatusUpdater as Lambda>::PersistedMemory,
    request: <OrderStatusUpdater as Lambda>::InputBody,
) -> Result<()> {
    if let Some(ref current_state) = request.payload.current_state {
        // If it is a locking state, we need to remove the lock
        if current_state.is_locking_state() {
            state
                .orders_repository
                .update_order_state_and_unlock_address(
                    state.cache_table_name.clone(),
                    request.payload.order_id.to_string(),
                    request.payload.next_state,
                    request.payload.update_order_statement,
                )
                .await
                .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
        }
        // Otherwise we just change the state and nothing else
        else {
            state
                .orders_repository
                .update_order_status_and_execution_id_non_terminal_state(
                    request.payload.order_id.to_string(),
                    request.payload.next_state,
                    request.payload.update_order_statement,
                )
                .await
                .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
        }

        Ok(())
    } else {
        Err(OrchestrationError::Validation(
            "trying to transition to Error state without passing current state argument".to_owned(),
        ))
    }
}

// Same behaviour of process_error_next_state but also updating the replaced order
async fn process_error_next_state_with_replacement(
    state: &<OrderStatusUpdater as Lambda>::PersistedMemory,
    request: <OrderStatusUpdater as Lambda>::InputBody,
    replaces_order_id: String,
) -> Result<()> {
    if let Some(ref current_state) = request.payload.current_state {
        // If it is a locking state, we need to remove the lock
        if current_state.is_locking_state() {
            state
                .orders_repository
                .update_order_state_with_replacement_and_unlock_address(
                    state.cache_table_name.clone(),
                    request.payload.order_id.to_string(),
                    replaces_order_id,
                    request.payload.next_state,
                    request.payload.update_order_statement,
                )
                .await
                .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
        }
        // Otherwise we just change the state and nothing else
        else {
            state
                .orders_repository
                .update_order_status_with_replacement_and_execution_id_non_terminal_state(
                    request.payload.order_id.to_string(),
                    replaces_order_id,
                    request.payload.next_state,
                    request.payload.update_order_statement,
                )
                .await
                .map_err(|e| report_and_return_error(e, request.payload.order_id))?;
        }

        Ok(())
    } else {
        Err(OrchestrationError::Validation(
            "trying to transition to Error state without passing current state argument".to_owned(),
        ))
    }
}

lambda_main!(OrderStatusUpdater);

#[cfg(test)]
mod tests {}
