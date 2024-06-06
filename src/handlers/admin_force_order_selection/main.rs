use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::event_bridge::get_event_bridge_client;
use mpc_signature_sm::publish::config::EbConfig;
use mpc_signature_sm::publish::{EventBridgePublisher, EventPublisher};
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::config::Config;
use crate::dtos::{AdminForceOrderSelectionRequest, AdminForceOrderSelectionResponse};

mod config;
mod dtos;

type EventPublisherObject = Arc<dyn EventPublisher + Sync + Send>;

pub struct Persisted {
    pub config: Config,
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub event_publisher: EventPublisherObject,
}
pub struct AdminForceOrderSelection;

#[async_trait]
impl Lambda for AdminForceOrderSelection {
    type PersistedMemory = Persisted;
    type InputBody = AdminForceOrderSelectionRequest;
    type Output = AdminForceOrderSelectionResponse;
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let eb_config = ConfigLoader::load_default::<EbConfig>();
        let dynamodb_client = get_dynamodb_client();

        let event_publisher = Arc::new(EventBridgePublisher::new(
            &eb_config,
            get_event_bridge_client(),
        )) as EventPublisherObject;

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            config,
            orders_repository,
            event_publisher,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        tracing::info!(
            order_id = ?request.order_id,
            "Forcing order selector for order id {}",
            request.order_id.to_string()
        );

        let order = state
            .orders_repository
            .get_order_by_id(request.order_id.to_string())
            .await
            .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

        state
            .event_publisher
            .publish_admin_force_order_event(
                order.clone().order_id,
                state.config.environment.clone(),
            )
            .await
            .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

        Ok(AdminForceOrderSelectionResponse {})
    }
}

lambda_main!(AdminForceOrderSelection);

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use async_trait::async_trait;
    use ethers::types::Transaction;
    use mockall::mock;
    use mockall::predicate::eq;
    use rstest::*;
    use uuid::Uuid;

    use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
    use model::order::helpers::build_signature_order;
    use model::order::OrderState;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::publish::{event_bridge::EventBridgeError, EventPublisher};
    use mpc_signature_sm::result::error::LambdaError;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use repositories::orders::OrdersRepositoryError;

    use crate::config::Config;
    use crate::dtos::AdminForceOrderSelectionRequest;
    use crate::{AdminForceOrderSelection, Persisted};

    mock! {
        EBPublisher {}
        #[async_trait]
        impl EventPublisher for EBPublisher {
            async fn publish_dropped_order_event(&self, order_id: Uuid) -> Result<(), EventBridgeError>;
            async fn publish_admin_force_order_event(&self, order_id: Uuid, environment: String,) -> Result<(), EventBridgeError>;
            async fn publish_stale_order_found_event(&self, order_id: Uuid, order_state: OrderState, environment: String,) -> Result<(), EventBridgeError>;
            async fn publish_transaction_event(&self,transaction_details: Transaction,chain_id: u64,order_id: Uuid) -> Result<(), EventBridgeError>;
        }
    }

    struct TestFixture {
        pub config: Config,
        pub orders_repository: MockOrdersRepository,
        pub event_publisher: MockEBPublisher,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = Config {
            order_status_table_name: "order-status".to_string(),
            aws_region: "us-west-2".to_owned(),
            environment: "tst".to_owned(),
        };
        TestFixture {
            config,
            orders_repository: MockOrdersRepository::new(),
            event_publisher: MockEBPublisher::new(),
        }
    }

    fn build_input(order_id: &str) -> AdminForceOrderSelectionRequest {
        AdminForceOrderSelectionRequest {
            order_id: Uuid::from_str(order_id).unwrap_or_default(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn admin_force_order_selection_ok(mut fixture: TestFixture) {
        let request = build_input(ORDER_ID_FOR_MOCK_REQUESTS);

        fixture
            .event_publisher
            .expect_publish_admin_force_order_event()
            .with(eq(request.order_id), eq("tst".to_owned()))
            .times(1)
            .returning(move |_, _| Ok(()));

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(request.order_id.to_string()))
            .once()
            .returning(move |_| {
                Ok(build_signature_order(
                    request.order_id,
                    OrderState::Signed,
                    None,
                ))
            });

        AdminForceOrderSelection::run(
            request.clone(),
            &Persisted {
                config: fixture.config,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                event_publisher: Arc::new(fixture.event_publisher),
            },
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn admin_force_order_selection_not_found(mut fixture: TestFixture) {
        let request = build_input(ORDER_ID_FOR_MOCK_REQUESTS);
        fixture
            .event_publisher
            .expect_publish_admin_force_order_event()
            .never();

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(request.order_id.to_string()))
            .once()
            .returning(move |_| {
                Err(OrdersRepositoryError::OrderNotFound(format!(
                    "Order with id {} not found",
                    request.order_id
                )))
            });

        let result = AdminForceOrderSelection::run(
            request.clone(),
            &Persisted {
                config: fixture.config,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                event_publisher: Arc::new(fixture.event_publisher),
            },
        )
        .await;

        assert!(result.is_err());
        let orc_error = result.unwrap_err();
        assert!(matches!(orc_error, LambdaError::Unknown(_)));
        assert!(orc_error
            .to_string()
            .contains(format!("Order with id {:} not found", request.order_id).as_str()));
    }
}
