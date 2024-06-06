use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}
