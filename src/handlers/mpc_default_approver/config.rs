use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    /// Current AWS region.
    pub aws_region: String,

    /// Queue's URL where the response will be stored
    pub response_queue_url: String,

    /// Name of the secret in Secrets Manager that holds the private key
    pub approver_private_key_secret_name: String,

    /// Delay in second before sending SQS message
    #[serde(default = "default_send_sqs_message_wait_seconds")]
    pub send_sqs_message_wait_seconds: u64,

    /// The name of the approver
    pub approver_name: String,

    /// Auto-Approve (approve) or Auto-Reject (reject)
    pub auto_approver_result: String,
}

pub enum AutoApproverResult {
    Approve,
    Reject,
}

impl From<AutoApproverResult> for i32 {
    fn from(value: AutoApproverResult) -> Self {
        match value {
            AutoApproverResult::Approve => 1,
            AutoApproverResult::Reject => 0,
        }
    }
}

impl FromStr for AutoApproverResult {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_uppercase(); // Convert the input to uppercase for case-insensitive comparison
        match s.as_str() {
            "APPROVE" => Ok(AutoApproverResult::Approve),
            _ => Ok(AutoApproverResult::Reject),
        }
    }
}

fn default_send_sqs_message_wait_seconds() -> u64 {
    0
}
