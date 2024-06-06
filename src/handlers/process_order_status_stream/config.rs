use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// Current Environment
    pub environment: String,
}
