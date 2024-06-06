use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::config::Config;
use crate::dtos::{requests::MpcOrderSelectorRequest, responses::MpcOrderSelectorResponse};
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use model::order::{OrderState, OrderStatus, OrderType};
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::Result,
};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

mod config;
mod dtos;

// TODO: Add Cancellation type when it exist (at the beginning so it gets processed first) -
// WALL-1583
const REPLACEMENT_ORDERS_TYPES: [OrderType; 2] = [OrderType::Cancellation, OrderType::SpeedUp];

pub struct Persisted {
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub order_age: Duration,
}

pub struct OrderSelectorRequest;

#[async_trait]
impl Lambda for OrderSelectorRequest {
    type PersistedMemory = Persisted;
    type InputBody = Event<MpcOrderSelectorRequest>;
    type Output = Event<MpcOrderSelectorResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name,
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            orders_repository,
            order_age: Duration::seconds(config.order_age_threshold_in_secs),
        })
    }

    async fn run(request: Self::InputBody, state: &Self::PersistedMemory) -> Result<Self::Output> {
        // First process the replacement types
        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            let result = process_replacement(&request, state, replacement_type).await?;
            if let Some(r) = result {
                return Ok(request.context.create_new_event_from_current(r));
            }
        }

        let result = process_sponsored(&request, state).await?;
        if let Some(r) = result {
            return Ok(request.context.create_new_event_from_current(r));
        }

        let result = process_submitted(&request, state).await?;
        if let Some(r) = result {
            return Ok(request.context.create_new_event_from_current(r));
        }

        let result = process_signed(&request, state).await?;
        if let Some(r) = result {
            return Ok(request.context.create_new_event_from_current(r));
        }

        let result = process_selected_for_signing(&request, state).await?;
        if let Some(r) = result {
            return Ok(request.context.create_new_event_from_current(r));
        }

        let result = process_approvers_reviewed(&request, state).await?;
        Ok(request.context.create_new_event_from_current(result))
    }
}

async fn process_approvers_reviewed(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
) -> Result<MpcOrderSelectorResponse> {
    let approvers_reviewed_orders = get_orders(
        request,
        state,
        OrderType::Signature,
        OrderState::ApproversReviewed,
        None,
    )
    .await?;

    // if not exists any order APPROVERS_REVIEWED -> exit
    if approvers_reviewed_orders.is_empty() {
        tracing::info!("NO ORDER SELECTED - not found order in APPROVERS_REVIEWED state");
        return Ok(MpcOrderSelectorResponse::OrderNotSelected {
            message: "APPROVERS_REVIEWED orders not found".to_string(),
        });
    }

    let selected_order = select_order(&approvers_reviewed_orders).await?;

    tracing::info!(
        order_id = ?selected_order.order_id,
        "SELECTED oldest APPROVERS_REVIEWED order with id {}",
        selected_order.order_id,
    );

    Ok(MpcOrderSelectorResponse::OrderInformation {
        order_id: selected_order.order_id,
        order_state: selected_order.state,
        order_type: selected_order.order_type,
    })
}

async fn process_selected_for_signing(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
) -> Result<Option<MpcOrderSelectorResponse>> {
    let selected_for_signing_orders = get_orders(
        request,
        state,
        OrderType::Signature,
        OrderState::SelectedForSigning,
        None,
    )
    .await?;

    if !selected_for_signing_orders.is_empty() {
        let selected_order = select_order(&selected_for_signing_orders).await?;

        return if old_order(selected_order, state.order_age) {
            tracing::warn!(
                order_id = selected_order.order_id.to_string(),
                "SELECTED an old SELECTED_FOR_SIGNING order with id {}",
                selected_order.order_id,
            );
            Ok(Some(MpcOrderSelectorResponse::OrderInformation {
                order_id: selected_order.order_id,
                order_state: selected_order.state,
                order_type: selected_order.order_type,
            }))
        } else {
            tracing::info!(
                order_id = ?selected_order.order_id,
                "NO ORDER SELECTED - found order in SELECTED_FOR_SIGNING state but not old enough with id {}",
                selected_order.order_id
            );
            Ok(Some(MpcOrderSelectorResponse::OrderNotSelected {
                message: format!(
                    "A SELECTED_FOR_SIGNING order found - not old enough with id {}",
                    selected_order.order_id
                ),
            }))
        };
    }

    Ok(None)
}

async fn process_signed(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
) -> Result<Option<MpcOrderSelectorResponse>> {
    let signed_orders = get_orders(
        request,
        state,
        OrderType::Signature,
        OrderState::Signed,
        None,
    )
    .await?;

    if !signed_orders.is_empty() {
        let selected_order = select_order(&signed_orders).await?;

        return if old_order(selected_order, state.order_age) {
            tracing::warn!(
                order_id = selected_order.order_id.to_string(),
                "SELECTED an old SIGNED order with id {}",
                selected_order.order_id,
            );
            Ok(Some(MpcOrderSelectorResponse::OrderInformation {
                order_id: selected_order.order_id,
                order_state: selected_order.state,
                order_type: selected_order.order_type,
            }))
        } else {
            tracing::info!(
                order_id = ?selected_order.order_id,
                "NO ORDER SELECTED - found order in SIGNED state but not old enough with id {}",
                selected_order.order_id,
            );

            Ok(Some(MpcOrderSelectorResponse::OrderNotSelected {
                message: format!(
                    "A SIGNED order found - not old enough with id {}",
                    selected_order.order_id,
                ),
            }))
        };
    }
    Ok(None)
}

async fn process_submitted(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
) -> Result<Option<MpcOrderSelectorResponse>> {
    let submitted_orders = get_orders(
        request,
        state,
        OrderType::Signature,
        OrderState::Submitted,
        Some(1),
    )
    .await?;

    // if exists any order SUBMITTED -> exit
    if !submitted_orders.is_empty() {
        tracing::info!(
            order_id = ?submitted_orders[0].order_id,
            "NO ORDER SELECTED - found an order in SUBMITTED state with id {}",
            submitted_orders[0].order_id
        );
        return Ok(Some(MpcOrderSelectorResponse::OrderNotSelected {
            message: format!(
                "A SUBMITTED order found with id {}",
                submitted_orders[0].order_id
            ),
        }));
    }

    Ok(None)
}

/// This function process all replacement orders
async fn process_replacement(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
    replacement_type: OrderType,
) -> Result<Option<MpcOrderSelectorResponse>> {
    if !REPLACEMENT_ORDERS_TYPES.contains(&replacement_type) {
        return Err(OrchestrationError::unknown(format!(
            "invalid replacement type {replacement_type}"
        )));
    }

    let approvers_reviewed_speedup_orders = get_orders(
        request,
        state,
        replacement_type,
        OrderState::ApproversReviewed,
        None,
    )
    .await?;

    if !approvers_reviewed_speedup_orders.is_empty() {
        let selected_order = select_order(&approvers_reviewed_speedup_orders).await?;
        tracing::info!(
            order_id = ?selected_order.order_id,
            "SELECTED speed up order with id {} and state {}",
            selected_order.order_id,
            selected_order.state
        );

        return Ok(Some(MpcOrderSelectorResponse::OrderInformation {
            order_id: selected_order.order_id,
            order_state: selected_order.state,
            order_type: selected_order.order_type,
        }));
    }

    Ok(None)
}

async fn process_sponsored(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
) -> Result<Option<MpcOrderSelectorResponse>> {
    let approvers_reviewed_sponsored_orders = get_orders(
        request,
        state,
        OrderType::Sponsored,
        OrderState::ApproversReviewed,
        None,
    )
    .await?;

    if !approvers_reviewed_sponsored_orders.is_empty() {
        let selected_order = select_order(&approvers_reviewed_sponsored_orders).await?;
        tracing::info!(
            order_id = ?selected_order.order_id,
            "SELECTED sponsored order with id {} and state {}",
            selected_order.order_id,
            selected_order.state
        );

        return Ok(Some(MpcOrderSelectorResponse::OrderInformation {
            order_id: selected_order.order_id,
            order_state: selected_order.state,
            order_type: selected_order.order_type,
        }));
    }

    Ok(None)
}

async fn get_orders(
    request: &Event<MpcOrderSelectorRequest>,
    state: &Persisted,
    order_type: OrderType,
    order_state: OrderState,
    limit: Option<i64>,
) -> Result<Vec<OrderStatus>> {
    let orders = state
        .orders_repository
        .get_orders_by_key_chain_type_state(
            request.payload.key_id.to_string(),
            request.payload.chain_id,
            order_type,
            order_state,
            limit,
        )
        .await
        .map_err(|e| OrchestrationError::from(anyhow!(e)))?;
    Ok(orders)
}

async fn select_order(orders: &[OrderStatus]) -> Result<&OrderStatus> {
    //TODO Improve searching oldest order

    let order = orders
        .iter()
        .min_by_key(|a| a.created_at)
        .ok_or(OrchestrationError::unknown("Error finding order"))?;

    Ok(order)
}

fn old_order(order: &OrderStatus, order_age: Duration) -> bool {
    if Utc::now() - order.last_modified_at < order_age {
        return false;
    }
    true
}

lambda_main!(OrderSelectorRequest);

#[cfg(test)]
mod tests {
    use crate::dtos::{requests::MpcOrderSelectorRequest, responses::MpcOrderSelectorResponse};
    use crate::{OrderSelectorRequest, Persisted, REPLACEMENT_ORDERS_TYPES};
    use chrono::{DateTime, Duration, Utc};
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
        KEY_ID_FOR_MOCK_REQUESTS,
    };
    use ethers::types::{H160, U256};
    use mockall::predicate::eq;
    use model::order::{
        GenericOrderData, OrderData, OrderState, OrderStatus, OrderTransaction, OrderType,
        SharedOrderData, SignatureOrderData,
    };
    use mpc_signature_sm::lambda_structure::event::{Event, EventContext};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use rstest::*;
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    const ORDER_TOO_OLD_IN_SECS: i64 = 600;
    // 10 min
    const FRESH_ORDER_AGE_IN_SECS: i64 = 60; // 1 min

    struct TestFixture {
        pub orders_repository: MockOrdersRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            orders_repository: MockOrdersRepository::new(),
        }
    }

    fn create_order_status(
        order_id: Uuid,
        order_type: OrderType,
        order_state: OrderState,
        created_at: DateTime<Utc>,
    ) -> OrderStatus {
        create_order_status_with_dates(order_id, order_type, order_state, created_at, Utc::now())
    }

    fn create_order_status_with_dates(
        order_id: Uuid,
        order_type: OrderType,
        order_state: OrderState,
        created_at: DateTime<Utc>,
        last_modified_at: DateTime<Utc>,
    ) -> OrderStatus {
        let order_data = OrderData::<SignatureOrderData> {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            },
            data: SignatureOrderData {
                transaction: OrderTransaction::Legacy {
                    to: ADDRESS_FOR_MOCK_REQUESTS.to_string(),
                    gas: U256::from(1),
                    gas_price: U256::from(1),
                    value: U256::from(1),
                    data: Default::default(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    nonce: Some(U256::from(0)),
                },
                address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                maestro_signature: None,
                key_id: Uuid::from_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
            },
        };

        OrderStatus {
            order_id,
            order_version: "1".to_string(),
            state: order_state,
            transaction_hash: None,
            data: GenericOrderData {
                shared_data: SharedOrderData {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                },
                data: serde_json::to_value(order_data).unwrap(),
            },
            created_at,
            last_modified_at,
            order_type,
            replaces: None,
            replaced_by: None,
            error: None,
            policy: None,
            cancellation_requested: None,
        }
    }

    fn build_input() -> Event<MpcOrderSelectorRequest> {
        Event {
            payload: MpcOrderSelectorRequest {
                chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                key_id: Uuid::parse_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
            },
            context: EventContext {
                order_id: Uuid::new_v4(),
                order_timestamp: Utc::now(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_submitted_order_ok(mut fixture: TestFixture) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id = Uuid::new_v4();
        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![create_order_status(
                    order_id,
                    OrderType::Signature,
                    OrderState::Submitted,
                    Utc::now(),
                )])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation { .. } => {
                panic!("Should have not found an order")
            }
            MpcOrderSelectorResponse::OrderNotSelected { message } => {
                assert_eq!(
                    message,
                    format!("A SUBMITTED order found with id {}", order_id)
                );
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_without_approvers_reviewed_order_ok(mut fixture: TestFixture) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::SelectedForSigning),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation { .. } => {
                panic!("Should have not found an order")
            }
            MpcOrderSelectorResponse::OrderNotSelected { message } => {
                assert_eq!(message, "APPROVERS_REVIEWED orders not found".to_string());
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_one_approvers_reviewed_order_ok(mut fixture: TestFixture) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::SelectedForSigning),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id = Uuid::new_v4();
        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![create_order_status(
                    order_id,
                    OrderType::Signature,
                    OrderState::ApproversReviewed,
                    Utc::now(),
                )])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_many_approvers_reviewed_order_ok(mut fixture: TestFixture) {
        let request = build_input();
        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::SelectedForSigning),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::ApproversReviewed,
                        Utc::now(),
                    ),
                    create_order_status(
                        order_id_old,
                        OrderType::Signature,
                        OrderState::ApproversReviewed,
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::ApproversReviewed,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_many_fresh_selected_for_signing_orders_ok(
        mut fixture: TestFixture,
    ) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::SelectedForSigning),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        order_id_old,
                        OrderType::Signature,
                        OrderState::SelectedForSigning,
                        Utc::now() - Duration::seconds(FRESH_ORDER_AGE_IN_SECS),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::SelectedForSigning,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderNotSelected { message } => {
                assert_eq!(
                    format!(
                        "A SELECTED_FOR_SIGNING order found - not old enough with id {}",
                        order_id_old
                    ),
                    message
                );
            }
            MpcOrderSelectorResponse::OrderInformation { .. } => {
                panic!("Should have found no orders")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_many_stale_selected_for_signing_orders_ok(
        mut fixture: TestFixture,
    ) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::SelectedForSigning),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status_with_dates(
                        order_id_old,
                        OrderType::Signature,
                        OrderState::SelectedForSigning,
                        Utc::now() - Duration::days(1) - Duration::hours(1),
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status_with_dates(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::SelectedForSigning,
                        Utc::now() - Duration::days(1) + Duration::hours(1),
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_many_fresh_signed_orders_ok(mut fixture: TestFixture) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        order_id_old,
                        OrderType::Signature,
                        OrderState::Signed,
                        Utc::now() - Duration::seconds(FRESH_ORDER_AGE_IN_SECS),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::Signed,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderNotSelected { message } => {
                assert_eq!(
                    format!(
                        "A SIGNED order found - not old enough with id {}",
                        order_id_old
                    ),
                    message
                );
            }
            MpcOrderSelectorResponse::OrderInformation { .. } => {
                panic!("Should have found no orders")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_many_stale_signed_orders_ok(mut fixture: TestFixture) {
        let request = build_input();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Submitted),
                eq(Some(1)),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Signature),
                eq(OrderState::Signed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status_with_dates(
                        order_id_old,
                        OrderType::Signature,
                        OrderState::Signed,
                        Utc::now() - Duration::days(1) - Duration::hours(1),
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status_with_dates(
                        Uuid::new_v4(),
                        OrderType::Signature,
                        OrderState::Signed,
                        Utc::now() + Duration::days(1) - Duration::hours(1),
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_approvers_reviewed_cancellation_orders_ok(
        mut fixture: TestFixture,
    ) {
        let request = build_input();

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Cancellation),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        order_id_old,
                        OrderType::Cancellation,
                        OrderState::ApproversReviewed,
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Cancellation,
                        OrderState::ApproversReviewed,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_approvers_reviewed_speedups_orders_ok(mut fixture: TestFixture) {
        let request = build_input();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Cancellation),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| Ok(vec![]));

        let order_id_old = Uuid::new_v4();

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::SpeedUp),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        order_id_old,
                        OrderType::SpeedUp,
                        OrderState::ApproversReviewed,
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::SpeedUp,
                        OrderState::ApproversReviewed,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn order_selector_with_approvers_reviewed_sponsored_orders_ok(mut fixture: TestFixture) {
        let request = build_input();

        let order_id_old = Uuid::new_v4();

        for replacement_type in REPLACEMENT_ORDERS_TYPES {
            fixture
                .orders_repository
                .expect_get_orders_by_key_chain_type_state()
                .with(
                    eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                    eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                    eq(replacement_type),
                    eq(OrderState::ApproversReviewed),
                    eq(None),
                )
                .once()
                .returning(move |_, _, _, _, _| Ok(vec![]));
        }

        fixture
            .orders_repository
            .expect_get_orders_by_key_chain_type_state()
            .with(
                eq(KEY_ID_FOR_MOCK_REQUESTS.to_string()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
                eq(OrderType::Sponsored),
                eq(OrderState::ApproversReviewed),
                eq(None),
            )
            .once()
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    create_order_status(
                        order_id_old,
                        OrderType::Sponsored,
                        OrderState::ApproversReviewed,
                        Utc::now() - Duration::days(1),
                    ),
                    create_order_status(
                        Uuid::new_v4(),
                        OrderType::Sponsored,
                        OrderState::ApproversReviewed,
                        Utc::now() + Duration::days(1),
                    ),
                ])
            });

        let result = OrderSelectorRequest::run(
            request.clone(),
            &Persisted {
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
                order_age: Duration::seconds(ORDER_TOO_OLD_IN_SECS),
            },
        )
        .await
        .unwrap();

        match result.payload {
            MpcOrderSelectorResponse::OrderInformation {
                order_id: retrieved_order_id,
                ..
            } => {
                assert_eq!(order_id_old, retrieved_order_id);
            }
            MpcOrderSelectorResponse::OrderNotSelected { .. } => {
                panic!("Should have found an order")
            }
        }
    }
}
