use crate::fixtures::eventbridge::{event_bridge_fixture, EventBridgeFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::sqs::{sqs_fixture, SqsFixture};
use crate::helpers::lambda::LambdaResponse;
use ana_tools::config_loader::ConfigLoader;
use http::StatusCode;
use model::order::OrderState;

use rstest::*;
use rusoto_sqs::{ReceiveMessageRequest, Sqs};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const FUNCTION_NAME: &str = "process_order_status_stream";
const EVENT_TYPE: &str = "order_terminal_state";

#[derive(Deserialize)]
pub struct Config {
    pub environment: String,
    pub aws_region: String,
    pub event_bridge_event_bus_name: String,
}

pub struct LocalFixture {
    event_sources: Vec<String>,
}

#[fixture]
fn local_fixture() -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let event_sources = vec![
        format!("{}-state-changed-to-cancelled-event", config.environment),
        format!(
            "{}-state-changed-to-not_submitted-event",
            config.environment
        ),
        format!("{}-state-changed-to-not_signed-event", config.environment),
        format!("{}-state-changed-to-error-event", config.environment),
        format!(
            "{}-state-changed-to-approvers_reviewed-event",
            config.environment
        ),
        format!("{}-state-changed-to-completed-event", config.environment),
        format!(
            "{}-state-changed-to-completed_with_error-event",
            config.environment
        ),
        format!("{}-state-changed-to-dropped-event", config.environment),
    ];

    LocalFixture { event_sources }
}

fn build_input(
    order_id: Uuid,
    order_state: OrderState,
    event_name: Option<&str>,
    order_type: Option<&str>,
) -> Value {
    json!({
        "Records": [
            {
                "eventID": "7de3041dd709b024af6f29e4fa13d34c",
                "eventName": event_name.unwrap_or("MODIFY"),
                "eventVersion": "1.1",
                "eventSource": "aws:dynamodb",
                "awsRegion": "region",
                "dynamodb": {
                    "ApproximateCreationDateTime": 1479499740,
                    "Keys": {
                        "order_id": { "S": order_id.to_owned() }
                    },
                    "NewImage": {
                        "state": { "S": order_state.as_str() },
                        "order_type": { "S": order_type.unwrap_or("SIGNATURE_ORDER") }
                    },
                    "SequenceNumber": "13021600000000001596893679",
                    "SizeBytes": 112,
                    "StreamViewType": "NEW_IMAGE"
                },
                "eventSourceARN": "arn:aws:dynamodb:region:account ID:table/BarkTable/stream/2016-11-16T20:42:48.104"
            }
        ]
    })
}

#[rstest]
#[case::completed(OrderState::Completed)]
#[case::not_signed(OrderState::NotSigned)]
#[case::not_submitted(OrderState::NotSubmitted)]
#[case::error(OrderState::Error)]
#[case::completed_with_error(OrderState::CompletedWithError)]
#[case::dropped(OrderState::Dropped)]
#[case::cancelled(OrderState::Cancelled)]
#[case::approvers_reviewed(OrderState::ApproversReviewed)]
#[tokio::test(flavor = "multi_thread")]
pub async fn stream_event_terminal_state_sends_eb_event_ok(
    fixture: &LambdaFixture,
    #[future] event_bridge_fixture: EventBridgeFixture,
    sqs_fixture: &SqsFixture,
    local_fixture: LocalFixture,
    #[case] order_state: OrderState,
) {
    let event_bridge_fixture = event_bridge_fixture.await;
    event_bridge_fixture
        .set_rule_with_sources(&local_fixture.event_sources, &[EVENT_TYPE.to_owned()])
        .await;

    let order_id = Uuid::new_v4();
    let input = build_input(order_id, order_state, None, None);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    let queue_messages = sqs_fixture
        .sqs_client
        .receive_message(ReceiveMessageRequest {
            queue_url: event_bridge_fixture.queue_url.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .messages
        .unwrap();

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!(1, queue_messages.len());

    let message_body: Value =
        serde_json::from_str(queue_messages[0].body.as_ref().unwrap()).unwrap();

    assert_eq!(message_body["detail"]["order_id"], order_id.to_string());
}

#[rstest]
#[case::received(OrderState::Received)]
#[case::reorged(OrderState::Reorged)]
#[case::replaced(OrderState::Replaced)]
#[case::selected_for_signing(OrderState::SelectedForSigning)]
#[case::signed(OrderState::Signed)]
#[case::submitted(OrderState::Submitted)]
#[tokio::test(flavor = "multi_thread")]
pub async fn stream_event_non_terminal_state_does_not_sends_eb_event_ok(
    fixture: &LambdaFixture,
    #[future] event_bridge_fixture: EventBridgeFixture,
    sqs_fixture: &SqsFixture,
    local_fixture: LocalFixture,
    #[case] order_state: OrderState,
) {
    let event_bridge_fixture = event_bridge_fixture.await;
    event_bridge_fixture
        .set_rule_with_sources(&local_fixture.event_sources, &[EVENT_TYPE.to_owned()])
        .await;

    let order_id = Uuid::new_v4();
    let input = build_input(order_id, order_state, None, None);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    let queue_messages = sqs_fixture
        .sqs_client
        .receive_message(ReceiveMessageRequest {
            queue_url: event_bridge_fixture.queue_url.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .messages;

    assert_eq!(StatusCode::OK, response.status);
    assert!(queue_messages.is_none());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn stream_event_invalid_event_name_ok(
    fixture: &LambdaFixture,
    #[future] event_bridge_fixture: EventBridgeFixture,
    sqs_fixture: &SqsFixture,
    local_fixture: LocalFixture,
) {
    let event_bridge_fixture = event_bridge_fixture.await;
    event_bridge_fixture
        .set_rule_with_sources(&local_fixture.event_sources, &[EVENT_TYPE.to_owned()])
        .await;

    let order_id = Uuid::new_v4();
    let input = build_input(order_id, OrderState::Completed, Some("INSERT"), None);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    let queue_messages = sqs_fixture
        .sqs_client
        .receive_message(ReceiveMessageRequest {
            queue_url: event_bridge_fixture.queue_url.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .messages;

    assert_eq!(StatusCode::OK, response.status);
    assert!(queue_messages.is_none());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn stream_event_invalid_order_type_ok(
    fixture: &LambdaFixture,
    #[future] event_bridge_fixture: EventBridgeFixture,
    sqs_fixture: &SqsFixture,
    local_fixture: LocalFixture,
) {
    let event_bridge_fixture = event_bridge_fixture.await;
    event_bridge_fixture
        .set_rule_with_sources(&local_fixture.event_sources, &[EVENT_TYPE.to_owned()])
        .await;

    let order_id = Uuid::new_v4();
    let input = build_input(
        order_id,
        OrderState::Completed,
        None,
        Some("KEY_CREATION_ORDER"),
    );
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    let queue_messages = sqs_fixture
        .sqs_client
        .receive_message(ReceiveMessageRequest {
            queue_url: event_bridge_fixture.queue_url.clone(),
            ..Default::default()
        })
        .await
        .unwrap()
        .messages;

    assert_eq!(StatusCode::OK, response.status);
    assert!(queue_messages.is_none());
}
