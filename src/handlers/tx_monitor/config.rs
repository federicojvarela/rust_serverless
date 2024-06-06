use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub keys_table_name: String,
    pub order_status_table_name: String,
    pub order_age_threshold_in_secs: i64,
    pub cache_table_name: String,
    /// Current Environment
    pub environment: String,

    #[serde(default = "default_last_modified_threshold")]
    pub last_modified_threshold: i64,
}

fn default_last_modified_threshold() -> i64 {
    10
}
