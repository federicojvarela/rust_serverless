use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}
