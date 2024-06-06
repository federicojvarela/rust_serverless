mod config;
mod dtos;

use std::sync::Arc;

use crate::dtos::{SelectPolicyRequest, SelectPolicyResponse};
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::dynamodb::get_dynamodb_client;
use config::Config;
use ethers::types::Address;
use model::address_policy_registry::AddressPolicyRegistry;
use mpc_signature_sm::{
    lambda_main,
    lambda_structure::{event::Event, lambda_trait::Lambda},
    result::error::OrchestrationError,
};
use repositories::address_policy_registry::{
    address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl,
    AddressPolicyRegistryRepository,
};

pub struct State {
    address_policy_registry_repository: Arc<dyn AddressPolicyRegistryRepository>,
}

pub struct SelectPolicy;

#[async_trait]
impl Lambda for SelectPolicy {
    type PersistedMemory = State;
    type InputBody = Event<SelectPolicyRequest>;
    type Output = Event<SelectPolicyResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let dynamodb_client = get_dynamodb_client();
        let config = ConfigLoader::load_default::<Config>();
        let address_policy_registry_repository =
            Arc::new(AddressPolicyRegistryRepositoryImpl::new(
                config.address_policy_registry_table_name.clone(),
                dynamodb_client,
            ));

        Ok(State {
            address_policy_registry_repository,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        // First try to get the policy for the "to" address for this client and chain id.
        let address_policy = get_policy(
            state,
            request.payload.client_id.clone(),
            request.payload.chain_id,
            Some(request.payload.address),
        )
        .await?;

        // If the policy for that specific address is not found, then we look for the default
        // policy for this client and chain id.
        let policy_name = if let Some(policy) = address_policy {
            policy.policy
        } else {
            let default_policy = get_policy(
                state,
                request.payload.client_id.clone(),
                request.payload.chain_id,
                None,
            )
            .await?;

            if let Some(default_policy) = default_policy {
                default_policy.policy
            } else {
                return Err(OrchestrationError::from(anyhow!(
                    "there was no default policy configured for client {} and chain id {}",
                    request.payload.client_id,
                    request.payload.chain_id
                )));
            }
        };

        Ok(request
            .context
            .create_new_event_from_current(SelectPolicyResponse { policy_name }))
    }
}

async fn get_policy(
    state: &State,
    client_id: String,
    chain_id: u64,
    address: Option<Address>,
) -> Result<Option<AddressPolicyRegistry>, OrchestrationError> {
    state
        .address_policy_registry_repository
        .get_policy(client_id, chain_id, address)
        .await
        .map_err(|e| {
            OrchestrationError::from(
                anyhow!(e).context(format!("error looking up policy for {:?}", address)),
            )
        })
}

lambda_main!(SelectPolicy);

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use crate::dtos::SelectPolicyRequest;
    use crate::{SelectPolicy, State};
    use anyhow::anyhow;
    use chrono::{DateTime, Utc};
    use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
    use ethers::types::Address;
    use mockall::predicate::eq;
    use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryType};
    use mpc_signature_sm::lambda_structure::event::{Event, EventContext};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::OrchestrationError;
    use repositories::address_policy_registry::{
        AddressPolicyRegistryRepositoryError, MockAddressPolicyRegistryRepository,
    };
    use rstest::{fixture, rstest};
    use uuid::Uuid;

    struct TestFixture {
        pub mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository::new(),
        }
    }

    fn get_input_event(
        order_id: Option<Uuid>,
        order_timestamp: Option<DateTime<Utc>>,
    ) -> Event<SelectPolicyRequest> {
        Event {
            payload: SelectPolicyRequest {
                client_id: "EA".to_owned(),
                chain_id: 11155111,
                address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            },
            context: EventContext {
                order_id: order_id.unwrap_or_default(),
                order_timestamp: order_timestamp.unwrap_or_default(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn error_retrieving_policy(mut fixture: TestFixture) {
        let request = get_input_event(None, None);

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .returning(|_, _, _| {
                Err(AddressPolicyRegistryRepositoryError::Unknown(anyhow!(
                    "error!"
                )))
            });

        let result = SelectPolicy::run(
            request,
            &State {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                ),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(result, OrchestrationError::Unknown(_)));
        assert!(format!("{result:?}").contains("error looking up policy for"));
    }

    #[rstest]
    #[tokio::test]
    async fn retrieve_policy_address_ok(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let order_timestamp = Utc::now();
        let request = get_input_event(Some(order_id), Some(order_timestamp));

        let policy_name = "SomeApprover";
        let closure_client_id = request.payload.client_id.clone();
        let address = request.payload.address;

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .with(
                eq(request.payload.client_id.clone()),
                eq(request.payload.chain_id),
                eq(Some(address)),
            )
            .returning(move |_, _, _| {
                Ok(Some(AddressPolicyRegistry {
                    client_id: closure_client_id.clone(),
                    chain_id: request.payload.chain_id,
                    r#type: AddressPolicyRegistryType::AddressTo { address },
                    policy: policy_name.to_owned(),
                }))
            });

        let result = SelectPolicy::run(
            request,
            &State {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                ),
            },
        )
        .await
        .unwrap();

        assert_eq!(order_timestamp, result.context.order_timestamp);
        assert_eq!(order_id, result.context.order_id);
        assert_eq!(policy_name, result.payload.policy_name);
    }

    #[rstest]
    #[tokio::test]
    async fn retrieve_policy_default_ok(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let order_timestamp = Utc::now();
        let request = get_input_event(Some(order_id), Some(order_timestamp));

        let policy_name = "SomeApprover";
        let closure_client_id = request.payload.client_id.clone();
        let address = request.payload.address;

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .with(
                eq(request.payload.client_id.clone()),
                eq(request.payload.chain_id),
                eq(Some(address)),
            )
            .returning(move |_, _, _| Ok(None));

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .with(
                eq(request.payload.client_id.clone()),
                eq(request.payload.chain_id),
                eq(None),
            )
            .returning(move |_, _, _| {
                Ok(Some(AddressPolicyRegistry {
                    client_id: closure_client_id.clone(),
                    chain_id: request.payload.chain_id,
                    r#type: AddressPolicyRegistryType::AddressTo { address },
                    policy: policy_name.to_owned(),
                }))
            });

        let result = SelectPolicy::run(
            request,
            &State {
                address_policy_registry_repository: Arc::new(
                    fixture.mock_address_policy_registry_repository,
                ),
            },
        )
        .await
        .unwrap();

        assert_eq!(order_timestamp, result.context.order_timestamp);
        assert_eq!(order_id, result.context.order_id);
        assert_eq!(policy_name, result.payload.policy_name);
    }
}
