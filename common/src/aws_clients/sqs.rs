use crate::config::aws_client_config::AwsClientConfig;
use ana_tools::config_loader::ConfigLoader;
use rusoto_core::credential::EnvironmentProvider;
use rusoto_sqs::SqsClient;

pub fn get_sqs_client() -> SqsClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>();
    let request_dispatcher = rusoto_core::request::HttpClient::new()
        .unwrap_or_else(|e| panic!("Unable to build Rusoto HTTP Client: {e}"));

    SqsClient::new_with(
        request_dispatcher,
        EnvironmentProvider::default(),
        config.region(),
    )
}
