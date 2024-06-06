use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;
use secrets_provider::{implementations::aws::AwsSecretsProvider, SecretsProvider};

/// Initializes a secrets provider with the AWS client
pub async fn get_secrets_provider() -> impl SecretsProvider {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;

    match config.region() {
        rusoto_core::Region::Custom {
            ref name,
            ref endpoint,
        } => AwsSecretsProvider::new_at_endpoint(name, endpoint).await,
        other => AwsSecretsProvider::new(other.name().to_string()).await,
    }
}
