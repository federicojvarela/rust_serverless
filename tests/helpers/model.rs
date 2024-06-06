use chrono::{DateTime, Utc};

use uuid::Uuid;

use common::test_tools::http::constants::{CLIENT_ID_FOR_MOCK_REQUESTS, HASH_FOR_MOCK_REQUESTS};

use model::order::helpers::{signature_data, sponsored_data};
use model::order::{GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData};

fn build_generic_order(
    order_id: Uuid,
    order_type: OrderType,
    transaction_hash: Option<String>,
    order_state: OrderState,
    client_id: String,
    last_modified_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
) -> OrderStatus {
    let data = if order_type == OrderType::Sponsored {
        sponsored_data()
    } else {
        signature_data()
    };

    OrderStatus {
        order_id,
        order_version: "1".to_string(),
        order_type,
        transaction_hash,
        state: order_state,
        data: GenericOrderData {
            shared_data: SharedOrderData { client_id },
            data,
        },
        replaces: None,
        replaced_by: None,
        last_modified_at,
        created_at,
        error: None,
        policy: None,
        cancellation_requested: None,
    }
}

pub fn build_order(
    order_state: OrderState,
    order_type: OrderType,
    last_modified_at: DateTime<Utc>,
) -> OrderStatus {
    build_generic_order(
        Uuid::new_v4(),
        order_type,
        Some(HASH_FOR_MOCK_REQUESTS.to_string()),
        order_state,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        last_modified_at,
        Utc::now(),
    )
}
