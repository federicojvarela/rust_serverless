use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub order_status_table_name: String,

    pub order_age_threshold_in_secs: i64,
}
