use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub nonces_table_name: String,
    pub order_status_table_name: String,
}
