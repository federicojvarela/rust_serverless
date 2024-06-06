use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::StepFunctionConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub send_transaction_to_approvers_arn: String,

    pub keys_table_name: String,

    pub order_status_table_name: String,
}

impl From<&Config> for StepFunctionConfig {
    fn from(value: &Config) -> Self {
        StepFunctionConfig {
            step_function_arn: value.send_transaction_to_approvers_arn.clone(),
        }
    }
}
