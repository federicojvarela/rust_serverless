use crate::config::aws_client_config::AwsClientConfig;
use ana_tools::config_loader::ConfigLoader;
use rusoto_core::credential::EnvironmentProvider;
use rusoto_stepfunctions::StepFunctionsClient;

pub fn get_step_functions_client() -> StepFunctionsClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>();

    let request_dispatcher =
        rusoto_core::request::HttpClient::new().expect("Unable to build Rusoto HTTP Client");

    StepFunctionsClient::new_with(
        request_dispatcher,
        EnvironmentProvider::default(),
        config.region(),
    )
}
