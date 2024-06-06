use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::StepFunctionConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
    pub cache_table_name: String,
    pub send_transaction_to_approvers_arn: String,
    pub keys_table_name: String,
}

impl From<&Config> for StepFunctionConfig {
    fn from(config: &Config) -> Self {
        Self {
            step_function_arn: config.send_transaction_to_approvers_arn.clone(),
        }
    }
}
