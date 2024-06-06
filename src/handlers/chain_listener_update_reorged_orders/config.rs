use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub keys_table_name: String,
    pub cache_table_name: String,
}
