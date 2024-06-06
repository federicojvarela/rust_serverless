use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AdminCancelOrdersRequest {
    pub order_ids: Vec<Uuid>,
}

#[derive(Serialize, Debug)]
pub struct AdminCancelOrdersResponse {
    pub data: Vec<Uuid>,
    pub errors: Vec<Uuid>,
}
