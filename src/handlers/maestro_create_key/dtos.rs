use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
pub struct KeyCreationRequest {
    pub client_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MaestroKeyCreationResponse {
    pub key_id: Uuid,
    pub public_key: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct KeyCreationResponse {
    pub key_id: Uuid,
    pub public_key: String,
    pub address: String,
}
