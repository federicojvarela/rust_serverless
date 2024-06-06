use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub nonces_table_name: String,
    pub cache_table_name: String,
}
