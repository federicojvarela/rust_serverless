use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub api_gateway_rest_api_id: String,
    pub api_gateway_stage_name: String,
}
