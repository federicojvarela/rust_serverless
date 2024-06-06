use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
}
