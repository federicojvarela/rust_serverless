use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::StepFunctionConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub send_transaction_to_approvers_arn: String,

    pub keys_table_name: String,

    pub order_status_table_name: String,

    pub sponsor_address_config_table_name: String,
}

impl From<&Config> for StepFunctionConfig {
    fn from(config: &Config) -> Self {
        Self {
            step_function_arn: config.send_transaction_to_approvers_arn.clone(),
        }
    }
}
