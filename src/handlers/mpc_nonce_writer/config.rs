use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub nonces_table_name: String,
    pub keys_table_name: String,
}
