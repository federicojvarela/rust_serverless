use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;
use rusoto_core::credential::EnvironmentProvider;
use rusoto_stepfunctions::StepFunctionsClient;

pub async fn get_step_functions_client() -> StepFunctionsClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;

    let request_dispatcher =
        rusoto_core::request::HttpClient::new().expect("Unable to build Rusoto HTTP Client");

    StepFunctionsClient::new_with(
        request_dispatcher,
        EnvironmentProvider::default(),
        config.region(),
    )
}
