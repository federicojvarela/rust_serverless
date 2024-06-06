use crate::validations::address::{AddressValidator, AddressValidatorError};
use anyhow::anyhow;
use async_trait::async_trait;
use ethers::types::Address;
use repositories::keys::{KeysRepository, KeysRepositoryError};
use std::str::FromStr;

pub struct AddressValidatorImpl<R: KeysRepository> {
    keys_repository: R,
}

impl<R: KeysRepository> AddressValidatorImpl<R> {
    pub fn new(keys_repository: R) -> Self {
        Self { keys_repository }
    }
}

#[async_trait]
impl<R: KeysRepository> AddressValidator for AddressValidatorImpl<R> {
    async fn valid_from_address(&self, address: String) -> Result<bool, AddressValidatorError> {
        let address = Address::from_str(&address).map_err(|e| {
            AddressValidatorError::Unknown(anyhow!(e).context("Error parsing address: {address}"))
        })?;

        match self.keys_repository.get_key_by_address(address).await {
            Ok(_) => Ok(true),
            Err(e) => match e {
                KeysRepositoryError::Unknown(e) => Err(AddressValidatorError::Unknown(e)),
                KeysRepositoryError::KeyNotFound(_) => Ok(false),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use crate::validations::address::AddressValidatorError;
    use crate::validations::address::{address_validator::AddressValidatorImpl, AddressValidator};
    use anyhow::anyhow;
    use async_trait::async_trait;
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use ethers::types::Address;
    use mockall::mock;
    use mockall::predicate::eq;
    use model::key::Key;
    use repositories::keys::*;
    use rstest::*;
    use std::str::FromStr;
    use uuid::Uuid;

    mock! {
        KeysRepo {}
        #[async_trait]
        impl KeysRepository for KeysRepo {
            async fn get_key_by_address(&self, address: Address) -> Result<Key, KeysRepositoryError>;
        }
    }
    struct TestFixture {
        pub keys_repository: MockKeysRepo,
    }
    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            keys_repository: MockKeysRepo::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn validate_from_address_db_error(mut fixture: TestFixture) {
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(|_| Err(KeysRepositoryError::Unknown(anyhow!("timeout!"))));

        let error = AddressValidatorImpl::new(fixture.keys_repository)
            .valid_from_address(ADDRESS_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, AddressValidatorError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn validate_from_address_no_items_returned_by_db(mut fixture: TestFixture) {
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(|_| Err(KeysRepositoryError::KeyNotFound("not found".to_owned())));

        let valid = AddressValidatorImpl::new(fixture.keys_repository)
            .valid_from_address(ADDRESS_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap();

        assert!(!valid);
    }

    #[rstest]
    #[tokio::test]
    async fn validate_from_address_ok(mut fixture: TestFixture) {
        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(eq(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()))
            .once()
            .returning(|_| {
                Ok(Key {
                    key_id: Uuid::new_v4(),
                    address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                    client_user_id: "some.client.user.id".to_owned(),
                    created_at: Utc::now(),
                    order_type: "KEY_CREATION_ORDER".to_owned(),
                    order_version: "1".to_owned(),
                    owning_user_id: Uuid::new_v4(),
                    public_key: "some.public.key".to_owned(),
                })
            });

        let valid = AddressValidatorImpl::new(fixture.keys_repository)
            .valid_from_address(ADDRESS_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap();

        assert!(valid);
    }
}
