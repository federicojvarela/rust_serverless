use ethers::types::H160;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::deserializers::h160::h160;
use model::order::OrderState;
use repositories::orders::input_builder::UpdateOrderStatement;

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct MpcUpdateOrderStatusRequest {
    pub current_state: Option<OrderState>,
    pub next_state: OrderState,
    #[serde(default, deserialize_with = "h160")]
    pub address: H160,
    pub order_id: Uuid,
    pub update_order_statement: Option<UpdateOrderStatement>,
}
