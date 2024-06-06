use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;
use rusoto_core::credential::EnvironmentProvider;
use rusoto_sqs::SqsClient;

pub async fn get_sqs_client() -> SqsClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;
    let request_dispatcher = rusoto_core::request::HttpClient::new()
        .unwrap_or_else(|e| panic!("Unable to build Rusoto HTTP Client: {e}"));

    SqsClient::new_with(
        request_dispatcher,
        EnvironmentProvider::default(),
        config.region(),
    )
}
