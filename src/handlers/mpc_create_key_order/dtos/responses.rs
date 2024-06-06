use mpc_signature_sm::model::step_function::StepFunctionContext;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct KeyOrderStateMachinePayload {
    pub payload: Payload,
    pub context: StepFunctionContext,
}

#[derive(Serialize)]
pub struct Payload {
    pub client_user_id: String,
    pub owning_user_id: Uuid,
    pub client_id: String,
}
