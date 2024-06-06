use serde::Serialize;

use crate::model::step_function::StepFunctionContext;
use model::order::SignatureOrderData;

#[derive(Serialize)]
pub struct SendToApproversStateMachineRequest {
    pub context: StepFunctionContext,
    pub payload: SignatureOrderData,
}
