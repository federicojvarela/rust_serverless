use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub order_status_table_name: String,
    pub cache_table_name: String,
}