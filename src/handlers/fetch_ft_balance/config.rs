use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub keys_table_name: String,
    pub cache_table_name: String,
}
