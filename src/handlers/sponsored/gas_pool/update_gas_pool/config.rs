use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub sponsor_address_config_table_name: String,
}
