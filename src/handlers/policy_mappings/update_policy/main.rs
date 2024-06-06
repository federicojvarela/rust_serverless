use crate::config::Config;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::config::ConfigLoader;
use dtos::UpdatePolicyMappingRequest;
use http::{Response, StatusCode};
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::config::SupportedChain;
use mpc_signature_sm::dtos::requests::address_or_default_path_param::AddressOrDefaultPathParam;
use mpc_signature_sm::http::errors::{unknown_error_response, validation_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::maestro::maestro_bootstrap;
use mpc_signature_sm::maestro::state::MaestroState;
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use repositories::address_policy_registry::address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl;
use repositories::address_policy_registry::AddressPolicyRegistryRepository;
use std::sync::Arc;

mod config;
mod dtos;

pub const CHAIN_ID_PATH_PARAM: &str = "chain_id";
pub const ADDRESS_PATH_PARAM: &str = "address";

pub struct State<APRR: AddressPolicyRegistryRepository> {
    address_policy_registry_repository: Arc<APRR>,
    maestro: MaestroState,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client().await;

        let secrets_provider = get_secrets_provider().await;
        let maestro = maestro_bootstrap(secrets_provider)
            .await
            .expect("unable to initialize maestro");

        let address_policy_registry_repository =
            Arc::new(AddressPolicyRegistryRepositoryImpl::new(
                config.await.address_policy_registry_table_name.clone(),
                dynamodb_client,
            ));

        State {
            address_policy_registry_repository,
            maestro,
        }
    },
    update_policy,
    [validate_chain_id_is_supported, validate_content_type]
);

async fn update_policy(
    request: Request,
    state: &State<impl AddressPolicyRegistryRepository>,
) -> HttpLambdaResponse {
    let body = request.extract_body::<UpdatePolicyMappingRequest>()?;

    let client_id = request.extract_client_id()?;
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;
    let address: AddressOrDefaultPathParam = request.extract_path_param(ADDRESS_PATH_PARAM)?;

    if !chain_id.is_supported() {
        return Err(validation_error_response(
            format!("chain_id {chain_id} is not supported",),
            None,
        ));
    }

    if !check_policy_belongs_to_client(&client_id, &body.policy, &state.maestro).await? {
        return Err(validation_error_response(
            format!(r#"invalid policy "{}""#, body.policy),
            None,
        ));
    }

    state
        .address_policy_registry_repository
        .update_policy(client_id, chain_id, address.extract_address(), body.policy)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error saving address policy mapping. {e:?}"
            )))
        })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

async fn check_policy_belongs_to_client(
    client_id: &str,
    policy: &str,
    maestro: &MaestroState,
) -> Result<bool, Response<String>> {
    let response = maestro
        .http
        .get(format!(
            "{}/{client_id}/policy/{policy}",
            maestro.config.maestro_url
        ))
        .send()
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error with maestro's request: {e:?}"
            )))
        })?;

    Ok(StatusCode::OK == response.status())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    };
    use http::StatusCode;
    use lambda_http::{
        aws_lambda_events::apigw::ApiGatewayProxyRequestContext, request::RequestContext, Body,
        Request, RequestExt,
    };

    use mpc_signature_sm::{
        dtos::responses::http_error::LambdaErrorResponse,
        maestro::{
            config::MaestroConfig,
            session::{login, MaestroLoginInformation},
            state::MaestroState,
        },
        rest::middlewares::AuthenticationMiddleware,
    };
    use repositories::address_policy_registry::MockAddressPolicyRegistryRepository;
    use rstest::{fixture, rstest};
    use serde_json::{json, Value};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{update_policy, State, ADDRESS_PATH_PARAM, CHAIN_ID_PATH_PARAM};

    struct TestFixture {
        pub mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository,
        pub maestro: MaestroState,
        pub mock_server: MockServer,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        let mock_server = MockServer::start().await;
        let config = MaestroConfig {
            maestro_url: mock_server.uri(),
            service_name: "test".to_owned(),
            maestro_api_key_secret_name: "dummy_secret_name_api_key".to_owned(),
            maestro_tenant_name: "tenant".to_owned(),
        };

        let http_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(AuthenticationMiddleware::new(
                &login,
                Arc::new(MaestroLoginInformation {
                    maestro_url: config.maestro_url.clone(),
                    service_name: config.service_name.clone(),
                    maestro_api_key: "dummy_api_secret".to_owned(),
                    tenant_name: "tenant".to_owned(),
                }),
                Some("dummy_token".to_owned()),
            ))
            .build();

        TestFixture {
            mock_address_policy_registry_repository: MockAddressPolicyRegistryRepository::new(),
            maestro: MaestroState {
                http: http_client,
                config,
            },
            mock_server,
        }
    }

    fn build_request(address: &str, chain_id: u64, policy: &str) -> Request {
        let body = json!({
            "policy": policy
        })
        .to_string();

        request_with_params(Some(chain_id.to_string()), Some(address.to_string()), body)
    }

    fn request_with_params(
        chain_id: Option<String>,
        address: Option<String>,
        body: String,
    ) -> Request {
        let mut path_params: HashMap<String, String> = [].into();

        if let Some(chain_id) = chain_id {
            path_params.insert(CHAIN_ID_PATH_PARAM.to_string(), chain_id);
        }

        if let Some(address) = address {
            path_params.insert(ADDRESS_PATH_PARAM.to_string(), address);
        }

        let authorizer: HashMap<String, Value> = HashMap::from([(
            "claims".to_string(),
            json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS }),
        )]);

        let request_context = RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext {
            authorizer,
            ..ApiGatewayProxyRequestContext::default()
        });

        Request::new(Body::Text(body))
            .with_path_parameters::<HashMap<_, _>>(path_params)
            .with_request_context(request_context)
    }

    #[rstest]
    #[tokio::test]
    async fn update_invalid_address_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;

        let request = build_request("invalid_address", CHAIN_ID_FOR_MOCK_REQUESTS, "some_policy");

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = update_policy(request, &state).await.unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!("address with wrong type in request path", body.message);
    }

    #[rstest]
    #[tokio::test]
    async fn update_unssupported_chain_id_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let unssuported_chain_id = 919191919191919191;
        let policy_name = "some_policy";
        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, unssuported_chain_id, policy_name);

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = update_policy(request, &state).await.unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(
            format!("chain_id {unssuported_chain_id} is not supported"),
            body.message
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_invalid_policy_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let policy_name = "some_policy";
        let request = build_request(
            ADDRESS_FOR_MOCK_REQUESTS,
            CHAIN_ID_FOR_MOCK_REQUESTS,
            policy_name,
        );

        Mock::given(method("GET"))
            .and(path(format!(
                "/{CLIENT_ID_FOR_MOCK_REQUESTS}/policy/{policy_name}"
            )))
            .respond_with(ResponseTemplate::new(StatusCode::INTERNAL_SERVER_ERROR))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = update_policy(request, &state).await.unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(format!(r#"invalid policy "{policy_name}""#), body.message);
    }

    #[rstest]
    #[case::address(ADDRESS_FOR_MOCK_REQUESTS)]
    #[case::default("default")]
    #[tokio::test]
    async fn update_policy_ok(#[future] fixture: TestFixture, #[case] address: &str) {
        let mut fixture = fixture.await;
        let policy_name = "some_policy";
        let request = build_request(address, CHAIN_ID_FOR_MOCK_REQUESTS, "some_policy");

        fixture
            .mock_address_policy_registry_repository
            .expect_update_policy()
            .once()
            .returning(|_, _, _, _| Ok(()));

        Mock::given(method("GET"))
            .and(path(format!(
                "/{CLIENT_ID_FOR_MOCK_REQUESTS}/policy/{policy_name}"
            )))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "serialized_policy": "base64_policy",
                "policy_name": policy_name,
                "display_name": "Some Policy",
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = update_policy(request, &state).await.unwrap();

        assert_eq!(StatusCode::OK, response.status());
    }
}
