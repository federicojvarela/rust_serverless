use model::order::{OrderState, OrderType};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum MpcOrderSelectorResponse {
    OrderInformation {
        order_id: Uuid,
        order_state: OrderState,
        order_type: OrderType,
    },
    OrderNotSelected {
        message: String,
    },
}
