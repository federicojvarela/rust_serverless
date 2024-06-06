use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MpcOrderSelectorRequest {
    pub key_id: Uuid,
    pub chain_id: u64,
}
