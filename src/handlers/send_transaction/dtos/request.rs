use mpc_signature_sm::dtos::requests::transaction_request::TransactionRequest;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct SignedTransaction {
    pub transaction: TransactionRequest,
    pub key_id: String,
    pub approval_status: String,
    pub maestro_signature: String,
    pub transaction_hash: String,
}
