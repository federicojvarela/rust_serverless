use ana_tools::config_loader::ConfigLoader;
use common::{aws_clients::sqs::get_sqs_client, config::aws_client_config::AwsClientConfig};
use rstest::*;
use rusoto_events::{EventBridge, EventBridgeClient, PutRuleRequest, PutTargetsRequest, Target};
use rusoto_sqs::{CreateQueueRequest, Sqs, SqsClient};
use serde_json::json;
use uuid::Uuid;

const EVENT_BUS_NAME: &str = "default";

pub struct EventBridgeFixture {
    pub event_bridge_client: EventBridgeClient,
    pub sqs_client: SqsClient,
    pub queue_url: String,
    pub queue_name: String,
}

impl EventBridgeFixture {
    pub async fn set_rule_with_sources(&self, event_sources: &[String], event_types: &[String]) {
        self.event_bridge_client
            .put_rule(PutRuleRequest {
                event_bus_name: Some(EVENT_BUS_NAME.to_owned()),
                event_pattern: Some(
                    json!({
                        "source": event_sources.to_vec(),
                        "detail-type": event_types.to_vec()
                    })
                    .to_string(),
                ),
                name: "test-rule".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();

        self.event_bridge_client
            .put_targets(PutTargetsRequest {
                event_bus_name: Some(EVENT_BUS_NAME.to_owned()),
                rule: "test-rule".to_string(),
                targets: vec![Target {
                    arn: format!("arn:aws:sqs:us-west-2:000000000000:{}", self.queue_name),
                    id: Uuid::new_v4().to_string(),
                    ..Default::default()
                }],
            })
            .await
            .unwrap();
    }
}

pub fn get_event_bridge_client() -> EventBridgeClient {
    let config = ConfigLoader::load_default::<AwsClientConfig>();

    EventBridgeClient::new(config.region())
}

/// Since we can't query events from EventBridge, it sets up an SQS queue
/// as a target of the EventBridge. This allows us to then query SQS to verify the
/// events that were originally sent to EventBridge.
#[fixture]
pub async fn event_bridge_fixture() -> EventBridgeFixture {
    let sqs_client = get_sqs_client();

    // Different queue for every test
    let queue_name = format!("{}-event-bridge-events", Uuid::new_v4());
    let queue_url = sqs_client
        .create_queue(CreateQueueRequest {
            queue_name: queue_name.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .queue_url
        .unwrap();

    EventBridgeFixture {
        event_bridge_client: get_event_bridge_client(),
        sqs_client,
        queue_url,
        queue_name,
    }
}
