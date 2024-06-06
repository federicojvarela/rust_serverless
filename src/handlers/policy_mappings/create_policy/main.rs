use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::CreatePolicyMappingRequest;
use http::{Response, StatusCode};
use lambda_http::{run, service_fn, Error, Request};
use model::address_policy_registry::{AddressPolicyRegistry, AddressPolicyRegistryBuilder};
use mpc_signature_sm::feature_flags::FeatureFlags;
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
use repositories::address_policy_registry::address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl;
use repositories::address_policy_registry::AddressPolicyRegistryRepository;
use std::sync::Arc;
use validator::Validate;

mod config;
mod dtos;

pub struct State<APRR: AddressPolicyRegistryRepository> {
    address_policy_registry_repository: Arc<APRR>,
    maestro: MaestroState,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let secrets_provider = get_secrets_provider().await;
        let maestro = maestro_bootstrap(secrets_provider)
            .await
            .expect("unable to initialize maestro");

        let address_policy_registry_repository =
            Arc::new(AddressPolicyRegistryRepositoryImpl::new(
                config.address_policy_registry_table_name.clone(),
                dynamodb_client,
            ));

        State {
            address_policy_registry_repository,
            maestro,
        }
    },
    create_policy,
    [validate_content_type]
);

async fn create_policy(
    request: Request,
    state: &State<impl AddressPolicyRegistryRepository>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let body = request.extract_body::<CreatePolicyMappingRequest>()?;
    body.validate()
        .map_err(|e| validation_error_response(e.to_string(), None))?;

    let client_id = request.extract_client_id()?;

    if !check_policy_belongs_to_client(&client_id, &body.policy, &state.maestro).await? {
        return Err(validation_error_response(
            format!(r#"invalid policy "{}""#, body.policy),
            None,
        ));
    }

    let mapping = create_policy_mapping(client_id, body);

    state
        .address_policy_registry_repository
        .put_policy(mapping)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error saving address policy mapping. {e:?}"
            )))
        })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::CREATED,
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

fn create_policy_mapping(
    client_id: String,
    request_body: CreatePolicyMappingRequest,
) -> AddressPolicyRegistry {
    AddressPolicyRegistryBuilder::new(client_id, request_body.chain_id, request_body.policy)
        .address_to(request_body.address)
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
    use std::sync::Arc;

    use common::test_tools::http::{
        constants::{
            ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
        },
        helpers::build_request_custom_auth,
    };
    use http::{Request, StatusCode};
    use lambda_http::Body;
    use mpc_signature_sm::feature_flags::FeatureFlags;
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
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{create_policy, State};

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

    fn build_request(address: &str, chain_id: u64, policy: &str) -> Request<Body> {
        let body = Body::Text(
            json!({
                "address": address,
                "chain_id": chain_id,
                "policy": policy
            })
            .to_string(),
        );

        let auth = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS });
        build_request_custom_auth(auth, body)
    }

    #[rstest]
    #[tokio::test]
    async fn invalid_address_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;

        let request = build_request("invalid_address", CHAIN_ID_FOR_MOCK_REQUESTS, "some_policy");

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = create_policy(request, &state, &FeatureFlags::default_in_memory())
            .await
            .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(
            "Invalid H160 value: Invalid character 'i' at position 0 at line 1 column 28",
            body.message
        );
    }

    #[rstest]
    #[tokio::test]
    async fn unssupported_chain_id_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let unssuported_chain_id = 919191919191919191;
        let request = build_request(
            ADDRESS_FOR_MOCK_REQUESTS,
            unssuported_chain_id,
            "some_policy",
        );

        let state = State {
            address_policy_registry_repository: Arc::new(
                fixture.mock_address_policy_registry_repository,
            ),
            maestro: fixture.maestro,
        };

        let response = create_policy(request, &state, &FeatureFlags::default_in_memory())
            .await
            .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(
            format!(
                r#"chain_id: Validation error: chain_id is not supported [{{"value": Number({unssuported_chain_id})}}]"#
            ),
            body.message
        );
    }

    #[rstest]
    #[tokio::test]
    async fn invalid_policy_ok(#[future] fixture: TestFixture) {
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

        let response = create_policy(request, &state, &FeatureFlags::default_in_memory())
            .await
            .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(format!(r#"invalid policy "{policy_name}""#), body.message);
    }

    #[rstest]
    #[tokio::test]
    async fn create_policy_ok(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let policy_name = "some_policy";
        let request = build_request(
            ADDRESS_FOR_MOCK_REQUESTS,
            CHAIN_ID_FOR_MOCK_REQUESTS,
            "some_policy",
        );

        fixture
            .mock_address_policy_registry_repository
            .expect_put_policy()
            .once()
            .returning(|_| Ok(()));

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

        let response = create_policy(request, &state, &FeatureFlags::default_in_memory())
            .await
            .unwrap();

        assert_eq!(StatusCode::CREATED, response.status());
    }
}
