use async_trait::async_trait;
use repositories::orders::{OrdersRepository, OrdersRepositoryError};
use std::sync::Arc;
use uuid::Uuid;

use super::{errors::AuthorizationProviderError, AuthorizationProviderByOrder};

pub struct AuthorizationProviderByOrderImpl<R: OrdersRepository + Sync + Send> {
    orders_repository: Arc<R>,
}

impl<R: OrdersRepository + Sync + Send> AuthorizationProviderByOrderImpl<R> {
    pub fn new(orders_repository: Arc<R>) -> Self {
        Self { orders_repository }
    }
}

#[async_trait]
impl<R: OrdersRepository + Sync + Send> AuthorizationProviderByOrder
    for AuthorizationProviderByOrderImpl<R>
{
    async fn client_id_has_order_permission(
        &self,
        order_id: Uuid,
        client_id: &str,
    ) -> Result<bool, AuthorizationProviderError> {
        match self
            .orders_repository
            .get_order_by_id(order_id.to_string())
            .await
        {
            Ok(order) => {
                let allowed = order.data.shared_data.client_id == client_id;
                Ok(allowed)
            }
            Err(e) => match e {
                OrdersRepositoryError::Unknown(e) => Err(AuthorizationProviderError::Unknown(e)),
                OrdersRepositoryError::ConditionalCheckFailed(e) => {
                    Err(AuthorizationProviderError::Unknown(anyhow::anyhow!(e)))
                }
                OrdersRepositoryError::PreviousStatesNotFound => {
                    Err(AuthorizationProviderError::Unknown(anyhow::anyhow!(
                        "tried to transition state withouth checking previous state in code"
                    )))
                }
                OrdersRepositoryError::OrderNotFound(_) => Ok(false),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use anyhow::anyhow;
    use mockall::predicate::eq;
    use rstest::{fixture, rstest};
    use uuid::Uuid;

    use common::test_tools::http::constants::{
        CLIENT_ID_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
    };
    use model::order::helpers::build_signature_order;
    use model::order::OrderState;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepositoryError;

    use crate::authorization::AuthorizationProviderByOrder;
    use crate::authorization::{AuthorizationProviderByOrderImpl, AuthorizationProviderError};

    struct TestFixture {
        pub orders_repository: MockOrdersRepository,
    }
    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            orders_repository: MockOrdersRepository::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn check_order_permission_when_db_fails(mut fixture: TestFixture) {
        let order_id = Uuid::from_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Err(OrdersRepositoryError::Unknown(anyhow!("timeout!"))));

        let authorization_provider =
            AuthorizationProviderByOrderImpl::new(Arc::new(fixture.orders_repository));

        let error = authorization_provider
            .client_id_has_order_permission(order_id, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap_err();
        assert!(matches!(error, AuthorizationProviderError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn check_order_permission_not_found_order(mut fixture: TestFixture) {
        let order_id = Uuid::from_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| Err(OrdersRepositoryError::OrderNotFound("not found".to_owned())));

        let authorization_provider =
            AuthorizationProviderByOrderImpl::new(Arc::new(fixture.orders_repository));

        let allowed = authorization_provider
            .client_id_has_order_permission(order_id, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap();
        assert!(!allowed);
    }

    #[rstest]
    #[tokio::test]
    async fn check_address_permission_allowed(mut fixture: TestFixture) {
        let order_id = Uuid::from_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.to_string()))
            .once()
            .returning(move |_| {
                Ok(build_signature_order(
                    Uuid::from_str(ORDER_ID_FOR_MOCK_REQUESTS).unwrap(),
                    OrderState::Received,
                    None,
                ))
            });

        let authorization_provider =
            AuthorizationProviderByOrderImpl::new(Arc::new(fixture.orders_repository));

        let allowed = authorization_provider
            .client_id_has_order_permission(order_id, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap();
        assert!(allowed);
    }
}
