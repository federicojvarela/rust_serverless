mod config;
mod dtos;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::event_bridge::get_event_bridge_client;
use config::Config;
use dtos::{DynamoDbStreamEvent, DynamoDbStreamEventData};
use eventbridge_connector::{Event, EventBridge, EventBuilder};
use model::order::{OrderState, OrderType};
use mpc_signature_sm::publish::config::EbConfig;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub struct Persisted {
    pub event_bridge_client: Arc<dyn EventBridge>,
    pub eb_config: EbConfig,
    pub config: Config,
}

pub struct ProcessOrderStatusStream;

const MODIFY_STREAM_EVENT: &str = "MODIFY";
const STATES_TO_PROCESS: [OrderState; 8] = [
    OrderState::Cancelled,
    OrderState::Completed,
    OrderState::CompletedWithError,
    OrderState::Dropped,
    OrderState::Error,
    OrderState::NotSigned,
    OrderState::NotSubmitted,
    OrderState::ApproversReviewed,
];

#[async_trait]
impl Lambda for ProcessOrderStatusStream {
    type PersistedMemory = Persisted;
    type InputBody = DynamoDbStreamEvent;
    type Output = ();
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let eb_config = ConfigLoader::load_default::<EbConfig>();
        let config = ConfigLoader::load_default::<Config>();
        let event_bridge_client = Arc::new(get_event_bridge_client());

        Ok(Persisted {
            event_bridge_client,
            eb_config,
            config,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let mut not_submitted_orders = 0;
        let mut error_orders = 0;
        let events: Vec<Event> = request
            .records
            .into_iter()
            .map(|e| {
                build_terminal_state_eb_event(
                    e,
                    &state.config.environment,
                    &state.eb_config,
                    &mut not_submitted_orders,
                    &mut error_orders,
                )
            })
            .filter_map(|e| match e {
                Ok(e) => Some(e),
                Err(OrchestrationError::Validation(err)) => {
                    tracing::info!(error = ?err, "unprocessable event: {err}");
                    None
                }
                Err(err) => {
                    tracing::error!(error = ?err, "{err:?}");
                    None
                }
            })
            .collect();

        if !events.is_empty() {
            tracing::info!(
                not_submitted_orders,
                "Count of NotSubmitted orders: {not_submitted_orders}"
            );
            tracing::info!(error_orders, "Count of Error orders: {error_orders}");

            state
                .event_bridge_client
                .clone()
                .put_events(events)
                .await
                .map_err(|e| {
                    OrchestrationError::from(
                        anyhow!(e).context("unable to send event to event bridge"),
                    )
                })?;
        }

        Ok(())
    }
}

fn build_terminal_state_eb_event(
    mut event: DynamoDbStreamEventData,
    environment: &str,
    eb_config: &EbConfig,
    #[warn(unused_assignments)] not_submitted_orders: &mut i64,
    #[warn(unused_assignments)] error_orders: &mut i64,
) -> Result<Event, OrchestrationError> {
    if event.event_name != MODIFY_STREAM_EVENT {
        return Err(OrchestrationError::Validation(format!(
            "not a valid event \"{}\"",
            event.event_name
        )));
    }

    let order_id: Uuid =
        serde_dynamo::from_attribute_value(event.dynamodb.keys.remove("order_id").ok_or(
            OrchestrationError::Validation("order id not found in stream event".to_owned()),
        )?)
        .map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("unable to deserialize order id"))
        })?;

    if let Some(mut new_image) = event.dynamodb.new_image {
        let order_type: OrderType =
            serde_dynamo::from_attribute_value(new_image.remove("order_type").ok_or(
                OrchestrationError::Validation("order type not found in stream event".to_owned()),
            )?)
            .map_err(|e| {
                OrchestrationError::from(anyhow!(e).context(format!(
                    "order {order_id}: unable to deserialize order type",
                )))
            })?;

        if order_type == OrderType::KeyCreation {
            return Err(OrchestrationError::Validation(format!(
                "order {order_id}: {order_type} is not a valid order type"
            )));
        }

        let new_state: OrderState =
            serde_dynamo::from_attribute_value(new_image.remove("state").ok_or(
                OrchestrationError::Validation("state not found in stream event".to_owned()),
            )?)
            .map_err(|e| {
                OrchestrationError::from(anyhow!(e).context(format!(
                    "order {order_id}: unable to deserialize order state.",
                )))
            })?;

        if !STATES_TO_PROCESS.contains(&new_state) {
            return Err(OrchestrationError::Validation(format!(
                "order {order_id}: {new_state} is not a terminal state"
            )));
        }

        if new_state == OrderState::NotSubmitted {
            *not_submitted_orders += 1;
        }

        if new_state == OrderState::Error {
            *error_orders += 1;
        }

        return Ok(EventBuilder::new(
            json!({ "order_id": order_id }),
            "order_terminal_state",
            get_event_source(environment, new_state),
        )
        .event_bus_name(&eb_config.event_bridge_event_bus_name)
        .map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("unable to create event bridge event"))
        })?
        .build());
    }

    Err(OrchestrationError::Validation(format!(
        "order {order_id}: does not have a new_image field"
    )))
}

fn get_event_source(environment: &str, state: OrderState) -> String {
    format!(
        "{environment}-state-changed-to-{}-event",
        state.as_str().to_lowercase()
    )
}

lambda_main!(ProcessOrderStatusStream);

#[cfg(test)]
mod tests {
    use crate::{
        config::Config,
        dtos::{DynamoDbEvent, DynamoDbStreamEvent, DynamoDbStreamEventData},
        Persisted, ProcessOrderStatusStream, MODIFY_STREAM_EVENT, STATES_TO_PROCESS,
    };
    use eventbridge_connector::{tests::MockEventBridge, Event};
    use mockall::predicate;
    use model::order::OrderState;
    use model::order::OrderType;
    use mpc_signature_sm::{lambda_structure::lambda_trait::Lambda, publish::config::EbConfig};
    use rstest::*;
    use rusoto_core::Region;
    use serde::Serialize;
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestFixture {
        pub config: Config,
        pub eb_config: EbConfig,
        pub event_bridge_client: MockEventBridge,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = Config {
            environment: "test".to_owned(),
        };

        let eb_config = EbConfig {
            aws_region: Region::UsWest2,
            event_bridge_event_bus_name: "default".to_owned(),
        };

        TestFixture {
            config,
            eb_config,
            event_bridge_client: MockEventBridge::new(),
        }
    }

    #[derive(Serialize)]
    struct Key {
        order_id: String,
    }

    #[derive(Serialize)]
    struct NewState {
        state: String,
        order_type: String,
    }

    fn build_stream_event(
        order_id: Uuid,
        new_state: OrderState,
        order_type: OrderType,
    ) -> DynamoDbStreamEventData {
        DynamoDbStreamEventData {
            event_name: MODIFY_STREAM_EVENT.to_owned(),
            dynamodb: DynamoDbEvent {
                keys: serde_dynamo::to_item(Key {
                    order_id: order_id.to_string(),
                })
                .unwrap(),
                new_image: serde_dynamo::to_item(NewState {
                    state: new_state.to_string(),
                    order_type: order_type.to_string(),
                })
                .unwrap(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn process_order_status_stream_ok(mut fixture: TestFixture) {
        let request = DynamoDbStreamEvent {
            records: vec![
                build_stream_event(Uuid::new_v4(), OrderState::Cancelled, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::Completed, OrderType::Signature),
                build_stream_event(
                    Uuid::new_v4(),
                    OrderState::CompletedWithError,
                    OrderType::Signature,
                ),
                build_stream_event(
                    Uuid::new_v4(),
                    OrderState::ApproversReviewed,
                    OrderType::Signature,
                ),
                build_stream_event(Uuid::new_v4(), OrderState::Dropped, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::Error, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::NotSigned, OrderType::Signature),
                build_stream_event(
                    Uuid::new_v4(),
                    OrderState::NotSubmitted,
                    OrderType::Signature,
                ),
                build_stream_event(Uuid::new_v4(), OrderState::Received, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::Reorged, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::Replaced, OrderType::Signature),
                build_stream_event(
                    Uuid::new_v4(),
                    OrderState::SelectedForSigning,
                    OrderType::Signature,
                ),
                build_stream_event(Uuid::new_v4(), OrderState::Signed, OrderType::Signature),
                build_stream_event(Uuid::new_v4(), OrderState::Submitted, OrderType::Signature),
            ],
        };

        fixture
            .event_bridge_client
            .expect_put_events()
            .once()
            .with(predicate::function(|x: &Vec<Event>| {
                x.len() == STATES_TO_PROCESS.len()
            }))
            .returning(|_| Ok(()));

        let result = ProcessOrderStatusStream::run(
            request,
            &Persisted {
                config: fixture.config,
                eb_config: fixture.eb_config,
                event_bridge_client: Arc::new(fixture.event_bridge_client),
            },
        )
        .await;

        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn process_order_status_stream_ignore_not_modify_events_ok(mut fixture: TestFixture) {
        let request = DynamoDbStreamEvent {
            records: vec![DynamoDbStreamEventData {
                event_name: "CREATE".to_owned(),
                dynamodb: DynamoDbEvent {
                    keys: serde_dynamo::to_item(Key {
                        order_id: Uuid::new_v4().to_string(),
                    })
                    .unwrap(),
                    new_image: serde_dynamo::to_item(NewState {
                        state: OrderState::Completed.to_string(),
                        order_type: OrderType::Signature.to_string(),
                    })
                    .unwrap(),
                },
            }],
        };

        fixture.event_bridge_client.expect_put_events().never();

        let result = ProcessOrderStatusStream::run(
            request,
            &Persisted {
                config: fixture.config,
                eb_config: fixture.eb_config,
                event_bridge_client: Arc::new(fixture.event_bridge_client),
            },
        )
        .await;

        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn process_order_status_stream_ignore_key_creation_events_ok(mut fixture: TestFixture) {
        let request = DynamoDbStreamEvent {
            records: vec![DynamoDbStreamEventData {
                event_name: "MODIFY".to_owned(),
                dynamodb: DynamoDbEvent {
                    keys: serde_dynamo::to_item(Key {
                        order_id: Uuid::new_v4().to_string(),
                    })
                    .unwrap(),
                    new_image: serde_dynamo::to_item(NewState {
                        state: OrderState::Completed.to_string(),
                        order_type: OrderType::KeyCreation.to_string(),
                    })
                    .unwrap(),
                },
            }],
        };

        fixture.event_bridge_client.expect_put_events().never();

        let result = ProcessOrderStatusStream::run(
            request,
            &Persisted {
                config: fixture.config,
                eb_config: fixture.eb_config,
                event_bridge_client: Arc::new(fixture.event_bridge_client),
            },
        )
        .await;

        assert!(result.is_ok());
    }
}
