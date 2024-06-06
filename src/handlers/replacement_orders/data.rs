use anyhow::anyhow;
use chrono::Utc;
use http::Response;
use model::order::{
    GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData, SignatureOrderData,
};
use mpc_signature_sm::{http::errors::unknown_error_response, result::error::LambdaError};
use uuid::Uuid;

pub fn build_replacement_order(
    original_order: &OrderStatus,
    signature_order_data: &SignatureOrderData,
    order_type: OrderType,
) -> Result<OrderStatus, Response<String>> {
    let data = serde_json::to_value(signature_order_data).map_err(|e| {
        unknown_error_response(LambdaError::Unknown(
            anyhow!(e).context("Error serializing replacement order data"),
        ))
    })?;

    Ok(OrderStatus {
        order_id: Uuid::new_v4(),
        order_version: "1".to_string(),
        state: OrderState::Received,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: original_order.data.shared_data.client_id.clone(),
            },
            data,
        },
        created_at: Utc::now(),
        order_type,
        last_modified_at: Utc::now(),
        replaced_by: None,
        replaces: Some(original_order.order_id),
        error: None,
        policy: original_order.policy.clone(),
        cancellation_requested: None,
    })
}
