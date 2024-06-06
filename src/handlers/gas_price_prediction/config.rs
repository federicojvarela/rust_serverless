use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub cache_table_name: String,
}
