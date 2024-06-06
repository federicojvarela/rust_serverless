mod supported_chains;
use serde::{self, Deserialize};

pub use supported_chains::SupportedChain;

#[derive(Deserialize, Clone, Debug)]
pub struct GlobalConfig {
    /// Current AWS region.
    pub aws_region: String,

    /// Secret name holding LaunchDarkly's SDK key.
    pub launchdarkly_sdk_key_secret_name: Option<String>,

    /// Only used for development. Uses hardcoded in-memory feature flags.
    #[serde(default = "default_feature_flag_in_memory_mode")]
    pub feature_flag_in_memory_mode: bool,

    /// Only used for development. LocalStack endpoint
    #[serde(default = "default_localstack_test_mode_endpoint")]
    pub localstack_test_mode_endpoint: Option<String>,

    /// Only used for development. Step function / state machine endpoint
    #[serde(default = "default_step_function_test_mode_endpoint")]
    pub step_function_test_mode_endpoint: Option<String>,
}

fn default_feature_flag_in_memory_mode() -> bool {
    false
}

fn default_localstack_test_mode_endpoint() -> Option<String> {
    None
}

fn default_step_function_test_mode_endpoint() -> Option<String> {
    None
}
