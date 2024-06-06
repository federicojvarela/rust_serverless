use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::StepFunctionConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub key_creation_state_machine_arn: String,
}

impl From<Config> for StepFunctionConfig {
    fn from(config: Config) -> Self {
        Self {
            step_function_arn: config.key_creation_state_machine_arn,
        }
    }
}
