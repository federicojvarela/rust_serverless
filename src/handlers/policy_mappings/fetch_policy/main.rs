use std::sync::Arc;

use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::FetchPolicyResponse;
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::dtos::requests::address_or_default_path_param::AddressOrDefaultPathParam;
use mpc_signature_sm::http::errors::{not_found_response, unknown_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
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
    fetch_policy
);

async fn fetch_policy(
    request: Request,
    state: &State<impl AddressPolicyRegistryRepository>,
) -> HttpLambdaResponse {
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;
    let client_id = request.extract_client_id()?;
    let address = request
        .extract_path_param::<AddressOrDefaultPathParam>(ADDRESS_PATH_PARAM)?
        .extract_address();

    let policy = state
        .address_policy_registry_repository
        .get_policy(client_id, chain_id, address)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error fetching address policy mapping. {e:?}"
            )))
        })?
        .ok_or_else(|| {
            not_found_response(
                POLICY_NOT_FOUND_CODE,
                format!(
                    "there was no policy found for chain id {chain_id} and address {address:?}"
                ),
            )
        })?;

    let response = serde_json::to_string(&FetchPolicyResponse {
        policy: policy.policy,
    })
    .map_err(|e| {
        unknown_error_response(LambdaError::Unknown(
            anyhow::anyhow!(e).context("converting policy response"),
        ))
    })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        body: Some(response),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
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
    use mpc_signature_sm::dtos::responses::http_error::LambdaErrorResponse;
    use repositories::address_policy_registry::MockAddressPolicyRegistryRepository;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use std::str::FromStr;
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        dtos::FetchPolicyResponse, fetch_policy, State, ADDRESS_PATH_PARAM, CHAIN_ID_PATH_PARAM,
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
    async fn invalid_address_ok(mut fixture: TestFixture) {
        let request = build_request("invalid_address", CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .mock_address_policy_registry_repository
            .expect_delete_policy()
            .never();

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
        };

        let response = fetch_policy(request, &state).await.unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!("address with wrong type in request path", body.message);
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_policy_ok(mut fixture: TestFixture) {
        let policy = "some_policy";
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS);

        fixture
            .mock_address_policy_registry_repository
            .expect_get_policy()
            .once()
            .returning(|_, _, _| {
                Ok(Some(AddressPolicyRegistry {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                    chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                    policy: policy.to_owned(),
                    r#type: AddressPolicyRegistryType::AddressTo {
                        address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    },
                }))
            });

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
        };

        let response = fetch_policy(request, &state).await.unwrap();

        assert_eq!(StatusCode::OK, response.status());
        let body: FetchPolicyResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!(policy, body.policy);
    }
}
