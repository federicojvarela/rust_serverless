use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;
use rusoto_dynamodb::DynamoDbClient;

pub async fn get_dynamodb_client() -> DynamoDbClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;
    DynamoDbClient::new(config.region())
}
