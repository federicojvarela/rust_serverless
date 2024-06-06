use crate::authorization::AuthorizationProviderByAddress;
use async_trait::async_trait;
use ethers::types::Address;
use repositories::keys::{KeysRepository, KeysRepositoryError};
use std::sync::Arc;

use super::errors::AuthorizationProviderError;

pub struct AuthorizationProviderByAddressImpl<R: KeysRepository> {
    keys_repository: Arc<R>,
}

impl<R: KeysRepository> AuthorizationProviderByAddressImpl<R> {
    pub fn new(keys_repository: Arc<R>) -> Self {
        Self { keys_repository }
    }
}

#[async_trait]
impl<R: KeysRepository> AuthorizationProviderByAddress for AuthorizationProviderByAddressImpl<R> {
    async fn client_id_has_address_permission(
        &self,
        address: Address,
        client_id: &str,
    ) -> Result<bool, AuthorizationProviderError> {
        match self.keys_repository.get_key_by_address(address).await {
            Ok(key) => {
                let allowed = key.client_id == client_id;
                Ok(allowed)
            }
            Err(e) => match e {
                KeysRepositoryError::Unknown(e) => Err(AuthorizationProviderError::Unknown(e)),
                KeysRepositoryError::KeyNotFound(_) => Ok(false),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::authorization::authorization_provider_by_address::{
        AuthorizationProviderByAddressImpl, AuthorizationProviderError,
    };
    use crate::authorization::AuthorizationProviderByAddress;
    use anyhow::anyhow;
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, PUBLIC_KEY_FOR_MOCK_REQUESTS,
    };
    use ethers::types::Address;
    use mockall::predicate::eq;
    use model::key::Key;
    use repositories::keys::KeysRepositoryError;
    use repositories::keys::MockKeysRepository;
    use rstest::{fixture, rstest};
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestFixture {
        pub keys_repository: MockKeysRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            keys_repository: MockKeysRepository::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn check_address_permission_when_db_fails(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(move |_| Err(KeysRepositoryError::Unknown(anyhow!("timeout!"))));

        let authorization_provider =
            AuthorizationProviderByAddressImpl::new(Arc::new(fixture.keys_repository));

        let error = authorization_provider
            .client_id_has_address_permission(address, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap_err();
        assert!(matches!(error, AuthorizationProviderError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn check_address_permission_not_found_address(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(move |_| Err(KeysRepositoryError::KeyNotFound("not found".to_owned())));

        let authorization_provider =
            AuthorizationProviderByAddressImpl::new(Arc::new(fixture.keys_repository));

        let allowed = authorization_provider
            .client_id_has_address_permission(address, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap();
        assert!(!allowed);
    }

    #[rstest]
    #[tokio::test]
    async fn check_address_permission_allowed(mut fixture: TestFixture) {
        let address = Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap();
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(move |_| {
                Ok(Key {
                    key_id: Uuid::new_v4(),
                    order_type: "KEY_CREATION_ORDER".to_string(),
                    order_version: "1".to_string(),
                    client_user_id: Uuid::new_v4().to_string(),
                    address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                    owning_user_id: Uuid::new_v4(),
                    public_key: PUBLIC_KEY_FOR_MOCK_REQUESTS.to_string(),
                    created_at: Utc::now(),
                })
            });

        let authorization_provider =
            AuthorizationProviderByAddressImpl::new(Arc::new(fixture.keys_repository));

        let allowed = authorization_provider
            .client_id_has_address_permission(address, CLIENT_ID_FOR_MOCK_REQUESTS)
            .await
            .unwrap();
        assert!(allowed);
    }
}
