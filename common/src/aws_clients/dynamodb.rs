use crate::config::aws_client_config::AwsClientConfig;
use ana_tools::config_loader::ConfigLoader;
use rusoto_dynamodb::DynamoDbClient;

pub fn get_dynamodb_client() -> DynamoDbClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>();
    DynamoDbClient::new(config.region())
}
