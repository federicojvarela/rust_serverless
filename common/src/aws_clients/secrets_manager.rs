use crate::config::aws_client_config::AwsClientConfig;
use ana_tools::config_loader::ConfigLoader;
use secrets_provider::{implementations::aws::AwsSecretsProvider, SecretsProvider};

/// Initializes a secrets provider with the AWS client
pub async fn get_secrets_provider() -> impl SecretsProvider {
    let config = ConfigLoader::load_default::<AwsClientConfig>();

    match config.region() {
        rusoto_core::Region::Custom {
            ref name,
            ref endpoint,
        } => AwsSecretsProvider::new_at_endpoint(name, endpoint).await,
        other => AwsSecretsProvider::new(other.name().to_string()).await,
    }
}
