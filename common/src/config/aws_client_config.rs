use rusoto_core::region::Region;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct AwsClientConfig {
    /// Current AWS region.
    aws_region: String,

    /// Only used for development. LocalStack endpoint
    #[serde(default = "default_localstack_test_mode_endpoint")]
    pub localstack_test_mode_endpoint: Option<String>,
}

impl AwsClientConfig {
    pub fn region(&self) -> Region {
        if let Some(endpoint) = self.localstack_test_mode_endpoint.clone() {
            Region::Custom {
                name: self.aws_region.clone(),
                endpoint,
            }
        } else {
            Region::from_str(&self.aws_region).unwrap_or_else(|e| {
                panic!(
                    r#"Unable to parse AWS region "{}": {}"#,
                    &self.aws_region, e
                )
            })
        }
    }
}

fn default_localstack_test_mode_endpoint() -> Option<String> {
    None
}
