use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::serializers::h160::h160_to_lowercase_hex_string;
use dtos::{Address, Chain, FetchAllPolicyResponse, MappingType};
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request};
use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryType};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::unknown_error_response;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse,
};
use mpc_signature_sm::result::error::LambdaError;
use repositories::address_policy_registry::address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl;
use repositories::address_policy_registry::AddressPolicyRegistryRepository;

use crate::config::Config;

mod config;
mod dtos;

pub const POLICY_NOT_FOUND_CODE: &str = "policy_not_found";
pub const ADDRESS_PATH_PARAM: &str = "address";
pub const CHAIN_ID_PATH_PARAM: &str = "chain_id";

pub struct State<APRR: AddressPolicyRegistryRepository> {
    address_policy_registry_repository: Arc<APRR>,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let address_policy_registry_repository =
            Arc::new(AddressPolicyRegistryRepositoryImpl::new(
                config.address_policy_registry_table_name.clone(),
                dynamodb_client,
            ));

        State {
            address_policy_registry_repository,
        }
    },
    fetch_all_policy
);

async fn fetch_all_policy(
    request: Request,
    state: &State<impl AddressPolicyRegistryRepository>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let client_id = request.extract_client_id()?;

    let policies = state
        .address_policy_registry_repository
        .get_all_policies(client_id)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error fetching address policy mapping. {e:?}"
            )))
        })?;

    let fetch_policy_response = convert_policies_to_fetch_all_policy_response(policies);

    let final_response = serde_json::to_string(&fetch_policy_response).map_err(|e| {
        unknown_error_response(LambdaError::Unknown(
            anyhow::anyhow!(e).context("converting policy response"),
        ))
    })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        body: Some(final_response),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

fn convert_policies_to_fetch_all_policy_response(
    policies: Vec<AddressPolicyRegistry>,
) -> FetchAllPolicyResponse {
    let mut chains_map = std::collections::HashMap::new();

    for policy in policies {
        let chain_entry = chains_map.entry(policy.chain_id).or_insert_with(|| Chain {
            chain_id: policy.chain_id,
            addresses: Vec::new(),
        });

        let address = match policy.r#type {
            AddressPolicyRegistryType::Default => "default".to_owned(),
            AddressPolicyRegistryType::AddressTo { address } => {
                h160_to_lowercase_hex_string(address)
            }
            // TODO: At this stage this does not exist, we need to rethink the response dto
            AddressPolicyRegistryType::AddressFrom { address } => {
                h160_to_lowercase_hex_string(address)
            }
        };

        chain_entry.addresses.push(Address {
            address,
            policy: policy.policy,
            r#type: MappingType::from(policy.r#type),
        });
    }

    FetchAllPolicyResponse {
        chains: chains_map.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use common::test_tools::http::{
        constants::{
            ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
        },
        helpers::build_request_custom_auth,
    };
    use ethers::types::Address;
    use http::{Request, StatusCode};
    use lambda_http::{Body, RequestExt};
    use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryType};
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use repositories::address_policy_registry::MockAddressPolicyRegistryRepository;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use std::str::FromStr;
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        dtos::FetchAllPolicyResponse, fetch_all_policy, State, ADDRESS_PATH_PARAM,
        CHAIN_ID_PATH_PARAM,
    };

    struct TestFixture {
        pub mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository::new(),
        }
    }

    fn build_request(address: &str, chain_id: u64) -> Request<Body> {
        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        let mut path_params: HashMap<String, String> = HashMap::new();
        path_params.insert(CHAIN_ID_PATH_PARAM.to_owned(), chain_id.to_string());
        path_params.insert(ADDRESS_PATH_PARAM.to_owned(), address.to_owned());

        build_request_custom_auth(auth, Body::default()).with_path_parameters(path_params)
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_all_policy_ok(mut fixture: TestFixture) {
        let policy = "some policies";
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);
        let default_policy = "default_policy";
        let second_chain_id = CHAIN_ID_FOR_MOCK_REQUESTS + 1;

        fixture
            .mock_address_policy_registry_repository
            .expect_get_all_policies()
            .once()
            .returning(move |_| {
                Ok(vec![
                    AddressPolicyRegistry {
                        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                        chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                        policy: policy.to_owned(),
                        r#type: AddressPolicyRegistryType::AddressTo {
                            address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                        },
                    },
                    AddressPolicyRegistry {
                        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                        chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                        policy: default_policy.to_owned(),
                        r#type: AddressPolicyRegistryType::Default,
                    },
                    AddressPolicyRegistry {
                        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                        chain_id: second_chain_id, // This is now safe to use
                        policy: policy.to_owned(),
                        r#type: AddressPolicyRegistryType::AddressTo {
                            address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                        },
                    },
                    AddressPolicyRegistry {
                        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                        chain_id: second_chain_id, // This is now safe to use
                        policy: default_policy.to_owned(),
                        r#type: AddressPolicyRegistryType::Default,
                    },
                ])
            });

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
        };

        let response = fetch_all_policy(request, &state, &FeatureFlags::default_in_memory())
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());
        let body: FetchAllPolicyResponse = serde_json::from_str(response.body()).unwrap();

        assert_eq!(body.chains.len(), 2); // Check that there are two chains

        // Validate the first chain
        let first_chain = &body
            .chains
            .iter()
            .find(|c| c.chain_id == CHAIN_ID_FOR_MOCK_REQUESTS)
            .unwrap();
        assert_eq!(first_chain.addresses.len(), 2);

        // Validate the second chain
        let second_chain = body
            .chains
            .iter()
            .find(|c| c.chain_id == second_chain_id)
            .unwrap();
        assert_eq!(second_chain.addresses.len(), 2);
    }
}
