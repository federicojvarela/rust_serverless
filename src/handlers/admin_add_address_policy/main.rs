use std::sync::Arc;

use crate::config::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;

use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::requests::AdminAddAddressPolicyRequest;
use model::address_policy_registry::AddressPolicyRegistry;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};
use repositories::address_policy_registry::address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl;
use repositories::address_policy_registry::AddressPolicyRegistryRepository;

use crate::config::Config;

mod config;
mod dtos;

pub struct Persisted {
    pub address_policy_registry_repository: Arc<dyn AddressPolicyRegistryRepository>,
}
pub struct AdminAddAddressPolicy;

#[async_trait]
impl Lambda for AdminAddAddressPolicy {
    type PersistedMemory = Persisted;
    type InputBody = AdminAddAddressPolicyRequest;
    type Output = ();
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();
        let address_policy_registry_repository =
            Arc::new(AddressPolicyRegistryRepositoryImpl::new(
                config.address_policy_registry_table_name.clone(),
                dynamodb_client,
            ));

        Ok(Persisted {
            address_policy_registry_repository,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        if !request.force {
            let policy = state
                .address_policy_registry_repository
                .get_policy(request.client_id.clone(), request.chain_id, request.address)
                .await
                .map_err(|e| {
                    OrchestrationError::from(anyhow!(e).context("error getting policy"))
                })?;

            if policy.is_some() {
                return Err(OrchestrationError::from(anyhow!("policy already exists")));
            }
        }

        let mapping = AddressPolicyRegistry::from(request);

        _ = state
            .address_policy_registry_repository
            .put_policy(mapping)
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e).context("error adding new policy")))?;
        Ok(())
    }
}

lambda_main!(AdminAddAddressPolicy);

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use anyhow::anyhow;
    use ethers::prelude::H160;
    use ethers::types::Address;
    use mockall::predicate::eq;
    use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryType};
    use rstest::*;

    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::OrchestrationError;
    use repositories::address_policy_registry::MockAddressPolicyRegistryRepository;
    use repositories::address_policy_registry::{
        AddressPolicyRegistryRepository, AddressPolicyRegistryRepositoryError,
    };

    use crate::dtos::requests::AdminAddAddressPolicyRequest;
    use crate::AdminAddAddressPolicy;
    use crate::Persisted;

    struct TestFixture {
        pub mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository::new(),
        }
    }

    #[rstest]
    #[case::mock_address(Some(ADDRESS_FOR_MOCK_REQUESTS))]
    #[case::default_address(None)]
    #[tokio::test(flavor = "multi_thread")]
    async fn admin_add_address_policy_ok(mut fixture: TestFixture, #[case] address: Option<&str>) {
        let request = AdminAddAddressPolicyRequest {
            address: address.map(|addr| H160::from_str(addr).unwrap()),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            policy: "test-policy".to_string(),
            force: true,
        };

        let mapping = AddressPolicyRegistry::from(request.clone());

        fixture
            .mock_address_policy_registry_repository
            .expect_put_policy()
            .with(eq(mapping))
            .returning(move |_| Ok(()));

        let result = AdminAddAddressPolicy::run(
            request,
            &Persisted {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                )
                    as Arc<dyn AddressPolicyRegistryRepository>,
            },
        )
        .await;

        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn admin_add_address_policy_error(mut fixture: TestFixture) {
        let request = AdminAddAddressPolicyRequest {
            address: Some(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            policy: "test-policy".to_string(),
            force: true,
        };

        let mapping = AddressPolicyRegistry::from(request.clone());
        fixture
            .mock_address_policy_registry_repository
            .expect_put_policy()
            .with(eq(mapping))
            .returning(move |_| {
                Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                    "something went wrong"
                )))
            });

        let result = AdminAddAddressPolicy::run(
            request.clone(),
            &Persisted {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                )
                    as Arc<dyn AddressPolicyRegistryRepository>,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(result, OrchestrationError::Unknown(_)));
        assert!(format!("{result:?}").contains("error adding new policy"));
    }

    #[rstest]
    #[tokio::test]
    async fn admin_add_address_policy_error_already_exists(mut fixture: TestFixture) {
        let request = AdminAddAddressPolicyRequest {
            address: Some(H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            policy: "test-policy".to_string(),
            force: false,
        };

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .with(
                eq(request.client_id.to_owned()),
                eq(request.chain_id.to_owned()),
                eq(Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap())),
            )
            .returning(move |_, _, _| {
                Ok(Some(AddressPolicyRegistry {
                    r#type: AddressPolicyRegistryType::AddressTo {
                        address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    },
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                    policy: "test-policy".to_string(),
                }))
            });

        let mapping = AddressPolicyRegistry::from(request.clone());
        fixture
            .mock_address_policy_registry_repository
            .expect_put_policy()
            .with(eq(mapping))
            .returning(move |_| {
                Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                    "something went wrong"
                )))
            });

        let result = AdminAddAddressPolicy::run(
            request.clone(),
            &Persisted {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                )
                    as Arc<dyn AddressPolicyRegistryRepository>,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(result, OrchestrationError::Unknown(_)));
        assert!(format!("{result:?}").contains("policy already exists"));
    }
}
