use serde::Deserialize;

#[derive(Deserialize)]
pub struct LambdaConfig {
    /// URL where `cargo lambda watch` process is running
    pub lambda_watch_url: String,
}
