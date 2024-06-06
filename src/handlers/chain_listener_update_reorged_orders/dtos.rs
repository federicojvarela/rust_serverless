use common::deserializers::u64::str_u64;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct UpdateOrdersStatusEvent {
    pub detail: Detail,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Detail {
    pub hashes: Vec<String>,
    #[serde(rename = "chainId")]
    #[serde(deserialize_with = "str_u64")]
    pub chain_id: u64,
    #[serde(rename = "newState")]
    pub new_state: String,
}
