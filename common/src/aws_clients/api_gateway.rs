use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;
use rusoto_apigateway::{ApiGateway, ApiGatewayClient};

pub async fn get_api_gateway_client() -> impl ApiGateway {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;
    ApiGatewayClient::new(config.region())
}
