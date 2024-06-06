use crate::config::aws_client_config::AwsClientConfig;
use ana_tools::config_loader::ConfigLoader;
use rusoto_apigateway::{ApiGateway, ApiGatewayClient};

pub fn get_api_gateway_client() -> impl ApiGateway {
    let config = ConfigLoader::load_default::<AwsClientConfig>();
    ApiGatewayClient::new(config.region())
}
