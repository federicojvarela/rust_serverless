use crate::config::Config;
use crate::dtos::requests::{MpcNonceRequest, MpcNonceResponse};
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::dynamodb::get_dynamodb_client;
use ethers::types::U256;
use model::order::{OrderStatus, OrderTransaction, OrderType};
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::Result,
};
use repositories::nonces::nonces_repository_impl::NoncesRepositoryImpl;
use repositories::nonces::{NoncesRepository, NoncesRepositoryError};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;
use std::sync::Arc;

mod config;
mod dtos;

pub struct Persisted {
    pub nonces_repository: Arc<dyn NoncesRepository>,
    pub orders_repository: Arc<dyn OrdersRepository>,
}

pub struct FetchNonceRequest;

#[async_trait]
impl Lambda for FetchNonceRequest {
    type PersistedMemory = Persisted;
    type InputBody = Event<MpcNonceRequest>;
    type Output = Event<MpcNonceResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let nonces_repository = Arc::new(NoncesRepositoryImpl::new(
            config.nonces_table_name,
            dynamodb_client.clone(),
        )) as Arc<dyn NoncesRepository>;

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name,
            dynamodb_client,
        )) as Arc<dyn OrdersRepository>;

        Ok(Persisted {
            nonces_repository,
            orders_repository,
        })
    }

    async fn run(request: Self::InputBody, state: &Self::PersistedMemory) -> Result<Self::Output> {
        let order = state
            .orders_repository
            .get_order_by_id(request.context.order_id.to_string())
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e)))?;

        let nonce = match order.order_type {
            OrderType::Signature => nonce_from_nonce_manager(&request, state).await?,
            OrderType::SpeedUp | OrderType::Cancellation => {
                nonce_from_original_order(&order, state).await?
            }
            OrderType::Sponsored => nonce_for_meta_txn(&request).await?,
            _ => Err(OrchestrationError::from(anyhow!(
                "Cannot retrieve a nonce for a {} order",
                order.order_type.to_string()
            )))?,
        };

        Ok(Event {
            payload: nonce,
            context: request.context,
        })
    }
}

async fn nonce_from_nonce_manager(
    request: &Event<MpcNonceRequest>,
    state: &Persisted,
) -> Result<MpcNonceResponse> {
    let address = request.payload.address;
    let chain_id = request.payload.chain_id;

    let nonce = match state.nonces_repository.get_nonce(address, chain_id).await {
        Ok(nonce) => nonce.into(),
        Err(e) => match e {
            NoncesRepositoryError::NonceNotFound(_) => {
                MpcNonceResponse::zero_nonce(address, chain_id)
            }
            e => Err(OrchestrationError::from(anyhow!(e).context(format!(
                "Error retrieving Nonce for address {address}",
            ))))?,
        },
    };
    Ok(nonce)
}

async fn nonce_from_original_order(
    order: &OrderStatus,
    state: &Persisted,
) -> Result<MpcNonceResponse> {
    let original_order_id = order
        .replaces
        .ok_or(OrchestrationError::unknown("Missing order replaces"))?;

    let original_order = state
        .orders_repository
        .get_order_by_id(original_order_id.to_string())
        .await
        .map_err(|e| OrchestrationError::from(anyhow!(e)))?;

    let signature_order_data =
        original_order.extract_signature_data().map_err(|e| {
            OrchestrationError::from(anyhow!(e).context(
                "there was an error extracting the signature data from the original order",
            ))
        })?;

    let transaction_info: (Option<U256>, u64) = match signature_order_data.data.transaction {
        OrderTransaction::Eip1559 {
            nonce, chain_id, ..
        } => (nonce, chain_id),
        OrderTransaction::Legacy {
            nonce, chain_id, ..
        } => (nonce, chain_id),
        OrderTransaction::Sponsored { .. } => {
            return Err(OrchestrationError::unknown(
                "Sponsored orders do not have a nonce",
            ))
        }
    };

    let nonce = transaction_info
        .0
        .ok_or(OrchestrationError::unknown("Error getting nonce"))?;

    Ok(MpcNonceResponse {
        address: signature_order_data.data.address,
        nonce,
        chain_id: transaction_info.1,
        created_at: original_order.created_at,
        last_modified_at: original_order.last_modified_at,
    })
}

/*
    MOCKED FUNCTION: We do not store nonces for meta transactions as of yet.
    When we do, this function should be implemented and this mocked logic should be removed.
*/
async fn nonce_for_meta_txn(request: &Event<MpcNonceRequest>) -> Result<MpcNonceResponse> {
    let address = request.payload.address;
    let chain_id = request.payload.chain_id;

    Ok(MpcNonceResponse::zero_nonce(address, chain_id))
}

lambda_main!(FetchNonceRequest);

#[cfg(test)]
mod tests {
    use crate::dtos::requests::MpcNonceRequest;
    use crate::{FetchNonceRequest, Persisted};
    use anyhow::anyhow;
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use ethers::types::{Address, H160, U256};
    use mockall::predicate::eq;
    use model::nonce::Nonce;
    use model::order::{
        GenericOrderData, OrderData, OrderState, OrderStatus, OrderTransaction, OrderType,
        SharedOrderData, SignatureOrderData,
    };
    use mpc_signature_sm::lambda_structure::event::{Event, EventContext};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use repositories::nonces::*;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use rstest::*;
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestFixture {
        pub nonces_repository: MockNoncesRepository,
        pub orders_repository: MockOrdersRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            nonces_repository: MockNoncesRepository::new(),
            orders_repository: MockOrdersRepository::new(),
        }
    }

    fn create_order_status(
        order_id: Uuid,
        order_type: OrderType,
        replaces: Option<Uuid>,
        nonce: Option<U256>,
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
                    nonce,
                },
                address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                maestro_signature: None,
                key_id: Uuid::new_v4(),
            },
        };

        OrderStatus {
            order_id,
            order_version: "1".to_string(),
            state: OrderState::ApproversReviewed,
            transaction_hash: None,
            data: GenericOrderData {
                shared_data: SharedOrderData {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                },
                data: serde_json::to_value(order_data).unwrap(),
            },
            created_at: Utc::now(),
            order_type,
            last_modified_at: Utc::now(),
            replaces,
            replaced_by: None,
            error: None,
            policy: None,
            cancellation_requested: None,
        }
    }

    fn build_input() -> Event<MpcNonceRequest> {
        Event {
            payload: MpcNonceRequest {
                address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            },
            context: EventContext {
                order_id: Uuid::new_v4(),
                order_timestamp: Utc::now(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn db_error_when_fetching_nonce_signature(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::Signature,
                    None,
                    None,
                ))
            });

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Err(NoncesRepositoryError::Unknown(anyhow!("timeout!"))));

        let result = FetchNonceRequest::run(
            request,
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await;

        assert!(result.is_err());
        let error_message = result.err().unwrap().to_string();
        assert!(error_message.contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn entry_not_found_when_fetching_nonce_signature(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::Signature,
                    None,
                    None,
                ))
            });

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(|_, _| Err(NoncesRepositoryError::NonceNotFound("not found".to_owned())));

        let result = FetchNonceRequest::run(
            request.clone(),
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();
        let zero_as_u256: U256 = 0.into();

        assert_eq!(request.context, result.context);
        assert_eq!(request.payload.address, result.payload.address);
        assert_eq!(zero_as_u256, result.payload.nonce);
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_nonce_successfully_signature(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::Signature,
                    None,
                    None,
                ))
            });

        let address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let nonce = U256::from(5);
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let created_at = Utc::now();
        let last_modified_at = Utc::now();

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .once()
            .returning(move |_, _| {
                Ok(Nonce {
                    address,
                    chain_id,
                    nonce,
                    created_at,
                    last_modified_at,
                })
            });

        let result = FetchNonceRequest::run(
            request.clone(),
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();

        assert_eq!(request.context, result.context);
        assert_eq!(address, result.payload.address);
        assert_eq!(chain_id, result.payload.chain_id);
        assert_eq!(nonce, result.payload.nonce);
        assert_eq!(created_at, result.payload.created_at);
        assert_eq!(last_modified_at, result.payload.last_modified_at);
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_nonce_error_key_creation(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::KeyCreation,
                    None,
                    None,
                ))
            });

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .never();

        let result = FetchNonceRequest::run(
            request.clone(),
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await;

        assert!(result.is_err());
        let error_message = result.err().unwrap().to_string();
        assert!(error_message.contains("Cannot retrieve a nonce for a KEY_CREATION_ORDER order"));
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_nonce_successfully_speedup(mut fixture: TestFixture) {
        let original_order_id = Uuid::new_v4();
        let nonce = U256::from(25);
        let original_order =
            create_order_status(original_order_id, OrderType::Signature, None, Some(nonce));

        let address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        let chain_id = CHAIN_ID_FOR_MOCK_REQUESTS;
        let created_at = original_order.created_at;
        let last_modified_at = original_order.last_modified_at;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(original_order_id.clone().to_string()))
            .once()
            .returning(move |_| Ok(original_order.clone()));

        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::SpeedUp,
                    Some(original_order_id),
                    None,
                ))
            });

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .never();

        let result = FetchNonceRequest::run(
            request.clone(),
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await
        .unwrap();

        assert_eq!(request.context, result.context);
        assert_eq!(address, result.payload.address);
        assert_eq!(chain_id, result.payload.chain_id);
        assert_eq!(nonce, result.payload.nonce);
        assert_eq!(created_at, result.payload.created_at);
        assert_eq!(last_modified_at, result.payload.last_modified_at);
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_nonce_error_missing_replace_speedup(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| {
                Ok(create_order_status(
                    order_id,
                    OrderType::SpeedUp,
                    None,
                    None,
                ))
            });

        fixture
            .nonces_repository
            .expect_get_nonce()
            .with(
                eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
                eq(CHAIN_ID_FOR_MOCK_REQUESTS),
            )
            .never();

        let result = FetchNonceRequest::run(
            request.clone(),
            &Persisted {
                nonces_repository: Arc::new(fixture.nonces_repository) as Arc<dyn NoncesRepository>,
                orders_repository: Arc::new(fixture.orders_repository) as Arc<dyn OrdersRepository>,
            },
        )
        .await;

        assert!(result.is_err());
        let error_message = result.err().unwrap().to_string();
        assert!(error_message.contains("Missing order replaces"));
    }
}
