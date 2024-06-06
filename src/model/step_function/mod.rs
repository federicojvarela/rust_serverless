use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct StepFunctionContext {
    pub order_id: Uuid,
}
