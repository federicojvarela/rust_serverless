mod config;
mod dtos;

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::config::Config;
use crate::dtos::{AdminCancelOrdersRequest, AdminCancelOrdersResponse};
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

pub struct Persisted {
    pub config: Config,
    pub orders_repository: Arc<dyn OrdersRepository>,
}
pub struct AdminCancelOrders;

#[async_trait]
impl Lambda for AdminCancelOrders {
    type PersistedMemory = Persisted;
    type InputBody = AdminCancelOrdersRequest;
    type Output = AdminCancelOrdersResponse;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();
        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name.clone(),
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            config,
            orders_repository,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let mut data: Vec<Uuid> = Vec::new();
        let mut errors: Vec<Uuid> = Vec::new();

        for order_id in request.order_ids {
            tracing::info!(
                order_id = ?order_id,
                "Admin cancelling order id {}",
                order_id
            );

            let result = state
                .orders_repository
                .request_cancellation(order_id.to_string())
                .await;

            if result.is_ok() {
                data.push(order_id);
            } else {
                errors.push(order_id);

                if let Some(e) = result.err() {
                    tracing::error!(order_id = ?order_id,error = ?e, "Error trying to cancel order: {:?}", e)
                }
            }
        }

        Ok(AdminCancelOrdersResponse { data, errors })
    }
}

lambda_main!(AdminCancelOrders);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dtos::AdminCancelOrdersRequest;
    use crate::{AdminCancelOrders, Persisted};
    use common::test_tools::http::constants::ORDER_ID_FOR_MOCK_REQUESTS;
    use mockall::predicate::eq;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use repositories::orders::OrdersRepositoryError;
    use rstest::*;
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestFixture {
        pub config: Config,
        pub orders_repository: MockOrdersRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let config = Config {
            order_status_table_name: "order-status".to_string(),
        };
        TestFixture {
            config,
            orders_repository: MockOrdersRepository::new(),
        }
    }

    fn build_input(order_ids: Vec<Uuid>) -> AdminCancelOrdersRequest {
        AdminCancelOrdersRequest { order_ids }
    }

    #[rstest]
    #[tokio::test]
    async fn admin_cancel_orders_empty_orders_ok(fixture: TestFixture) {
        let request = build_input(vec![]);

        let result = AdminCancelOrders::run(
            request.clone(),
            &Persisted {
                config: fixture.config,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();

        assert!(result.data.is_empty());
        assert!(result.errors.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn admin_cancel_orders_missing_order_ok(mut fixture: TestFixture) {
        let order_id = Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
        let request = build_input(vec![order_id]);

        fixture
            .orders_repository
            .expect_request_cancellation()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| {
                Err(OrdersRepositoryError::OrderNotFound(format!(
                    "Order with id {} not found",
                    order_id
                )))
            });

        let result = AdminCancelOrders::run(
            request.clone(),
            &Persisted {
                config: fixture.config,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();

        assert!(result.data.is_empty());
        assert!(result.errors.contains(&order_id));
    }

    #[rstest]
    #[tokio::test]
    async fn admin_cancel_orders_existing_order_ok(mut fixture: TestFixture) {
        let order_id = Uuid::parse_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
        let request = build_input(vec![order_id]);

        fixture
            .orders_repository
            .expect_request_cancellation()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Ok(()));

        let result = AdminCancelOrders::run(
            request.clone(),
            &Persisted {
                config: fixture.config,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();

        assert!(result.data.contains(&order_id));
        assert!(result.errors.is_empty());
    }
}
