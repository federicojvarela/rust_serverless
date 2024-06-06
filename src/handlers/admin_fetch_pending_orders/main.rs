use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use crate::config::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use model::order::{OrderState, OrderSummary, OrderType};
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::result::error::Result as LambdaResult;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

use crate::config::Config;
use crate::dtos::models::{AdminFetchPendingOrdersRequest, AdminFetchPendingOrdersResponse};

mod config;
mod dtos;

pub struct Persisted {
    pub orders_repository: Arc<dyn OrdersRepository>,
}
pub struct AdminFetchPendingOrders;

#[async_trait]
impl Lambda for AdminFetchPendingOrders {
    type PersistedMemory = Persisted;
    type InputBody = AdminFetchPendingOrdersRequest;
    type Output = AdminFetchPendingOrdersResponse;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();
        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name,
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted { orders_repository })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let mut pending_orders: Vec<OrderSummary> = Vec::new();
        for order_type in OrderType::ORDER_TYPES_WITH_TXN {
            let mut orders_found = get_pending_orders_by_type(&request, state, order_type)
                .await
                .map_err(|e| OrchestrationError::from(anyhow!(e)))?;
            pending_orders.append(&mut orders_found);
        }

        let pending_orders_ids: Vec<String> = pending_orders
            .iter()
            .map(|order| order.order_id.to_string())
            .collect();

        Ok(AdminFetchPendingOrdersResponse {
            orders: pending_orders,
            order_ids: pending_orders_ids,
        })
    }
}

lambda_main!(AdminFetchPendingOrders);

async fn get_pending_orders_by_type(
    request: &AdminFetchPendingOrdersRequest,
    state: &Persisted,
    order_type: OrderType,
) -> LambdaResult<Vec<OrderSummary>> {
    let mut pending_orders: Vec<OrderSummary> = Vec::new();
    for order_state in OrderState::PENDING_ORDER_STATES {
        let mut orders_found = get_order_summaries(request, state, order_type, order_state).await?;
        pending_orders.append(&mut orders_found);
    }

    Ok(pending_orders)
}
async fn get_order_summaries(
    request: &AdminFetchPendingOrdersRequest,
    state: &Persisted,
    order_type: OrderType,
    order_state: OrderState,
) -> LambdaResult<Vec<OrderSummary>> {
    let orders = state
        .orders_repository
        .get_orders_summary_by_key_chain_type_state(
            request.key_id.to_string(),
            request.chain_id,
            order_type,
            order_state,
            None,
        )
        .await
        .map_err(|e| OrchestrationError::from(anyhow!(e)))?;
    Ok(orders)
}

#[cfg(test)]
mod tests {
    use mockall::predicate::eq;
    use model::order::OrderState;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use rstest::*;
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    use common::test_tools::http::constants::{
        CHAIN_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS,
    };
    use model::order::OrderType;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;

    use crate::dtos::models::AdminFetchPendingOrdersRequest;
    use crate::{AdminFetchPendingOrders, Persisted};

    struct TestFixture {
        pub orders_repository: MockOrdersRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            orders_repository: MockOrdersRepository::new(),
        }
    }

    fn build_input(key_id: &str, chain_id: u64) -> AdminFetchPendingOrdersRequest {
        AdminFetchPendingOrdersRequest {
            key_id: Uuid::from_str(key_id).unwrap_or_default(),
            chain_id,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn admin_fetch_pending_orders_ok(mut fixture: TestFixture) {
        for order_state in OrderState::PENDING_ORDER_STATES {
            fixture
                .orders_repository
                .expect_get_orders_summary_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(OrderType::Signature),
                    eq(order_state),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
            fixture
                .orders_repository
                .expect_get_orders_summary_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(OrderType::SpeedUp),
                    eq(order_state),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
            fixture
                .orders_repository
                .expect_get_orders_summary_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(OrderType::Cancellation),
                    eq(order_state),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
            fixture
                .orders_repository
                .expect_get_orders_summary_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(OrderType::Sponsored),
                    eq(order_state),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        let request = build_input(KEY_ID_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        let result = AdminFetchPendingOrders::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap().orders.is_empty());
    }
}
