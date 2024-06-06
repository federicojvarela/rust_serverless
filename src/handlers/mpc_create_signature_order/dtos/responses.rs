use ethers::types::H160;
use mpc_signature_sm::model::step_function::StepFunctionContext;
use serde::Serialize;
use uuid::Uuid;

use super::requests::TransactionRequest;

#[derive(Serialize)]
pub struct SignatureOrderStateMachinePayload {
    pub payload: Payload,
    pub key: Key,
    pub context: StepFunctionContext,
}

#[derive(Serialize)]
pub struct Payload {
    pub transaction: TransactionRequest,
    pub address: H160,
    pub client_id: String,
}

#[derive(Serialize)]
pub struct Key {
    pub key_id: Uuid,
}
