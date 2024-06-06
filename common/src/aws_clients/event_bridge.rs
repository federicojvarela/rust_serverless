use eventbridge_connector::EventBridge;
use eventbridge_connector::EventBridgeClient;

use crate::config::aws_client_config::AwsClientConfig;
use crate::config::ConfigLoader;

pub async fn get_event_bridge_client() -> impl EventBridge {
    let config = ConfigLoader::load_default::<AwsClientConfig>().await;

    match config.region() {
        rusoto_core::Region::Custom { name, endpoint } => {
            EventBridgeClient::new_at_endpoint(name, endpoint)
                .expect("unable to initialize event bridge client")
        }
        other => {
            EventBridgeClient::new(other.name()).expect("unable to initialize event bridge client")
        }
    }
}
