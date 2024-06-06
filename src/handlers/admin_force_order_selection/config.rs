use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,

    /// Current AWS region.
    pub aws_region: String,

    /// Current Environment
    pub environment: String,
}
