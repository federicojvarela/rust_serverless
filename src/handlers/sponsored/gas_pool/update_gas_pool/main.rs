use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::UpdateGasPoolRequest;
use ethers::types::Address;
use http::StatusCode;
use lambda_http::{run, service_fn, Error, Request};
use mpc_signature_sm::config::SupportedChain;
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{unknown_error_response, validation_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use mpc_signature_sm::validations::http::supported_chain_id::validate_chain_id_is_supported;
use repositories::sponsor_address_config::sponsor_address_config_repository_impl::SponsorAddressConfigRepositoryImpl;
use repositories::sponsor_address_config::SponsorAddressConfigRepository;
use std::sync::Arc;
use validator::Validate;

mod config;
mod dtos;

pub const CHAIN_ID_PATH_PARAM: &str = "chain_id";
pub const ADDRESS_PATH_PARAM: &str = "address";

// TODO: we should have a central place for error codes.
pub const ADDRESS_NOT_FOUND: &str = "address_not_found";

pub struct State<SACR: SponsorAddressConfigRepository> {
    sponsor_address_config_repository: Arc<SACR>,
}

http_lambda_main!(
    {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let sponsor_address_config_repository = Arc::new(SponsorAddressConfigRepositoryImpl::new(
            config.sponsor_address_config_table_name.clone(),
            dynamodb_client.clone(),
        ));

        State {
            sponsor_address_config_repository,
        }
    },
    update_gas_pool,
    [validate_chain_id_is_supported, validate_content_type]
);

async fn update_gas_pool(
    request: Request,
    state: &State<impl SponsorAddressConfigRepository>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let body = request.extract_body::<UpdateGasPoolRequest>()?;
    body.validate()
        .map_err(|e| validation_error_response(e.to_string(), None))?;

    let client_id = request.extract_client_id()?;
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;
    let _address: Address = request.extract_path_param(ADDRESS_PATH_PARAM)?;

    if !chain_id.is_supported() {
        return Err(validation_error_response(
            format!("chain_id {chain_id} is not supported",),
            None,
        ));
    }

    state
        .sponsor_address_config_repository
        .put_address_gas_pool(client_id, chain_id, body.gas_pool_address)
        .await
        .map_err(|e| {
            unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
                "there was an error updating gas pool address. {e:?}"
            )))
        })?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::OK,
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
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

    use mpc_signature_sm::dtos::responses::http_error::LambdaErrorResponse;
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use repositories::sponsor_address_config::MockSponsorAddressConfigRepository;
    use rstest::{fixture, rstest};
    use serde_json::{json, Value};

    use crate::{update_gas_pool, State, ADDRESS_PATH_PARAM, CHAIN_ID_PATH_PARAM};

    struct TestFixture {
        pub mock_sponsor_address_config_repository: MockSponsorAddressConfigRepository,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        TestFixture {
            mock_sponsor_address_config_repository: MockSponsorAddressConfigRepository::new(),
        }
    }

    impl TestFixture {
        pub fn get_state(self) -> State<MockSponsorAddressConfigRepository> {
            State {
                sponsor_address_config_repository: Arc::new(
                    self.mock_sponsor_address_config_repository,
                ),
            }
        }
    }

    fn build_request(address: &str, chain_id: u64, replacement_address: &str) -> Request {
        let body = json!({ "gas_pool_address": replacement_address }).to_string();

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

        let request = build_request(
            "invalid_address",
            CHAIN_ID_FOR_MOCK_REQUESTS,
            ADDRESS_FOR_MOCK_REQUESTS,
        );

        let response = update_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!("address with wrong type in request path", body.message);
    }

    #[rstest]
    #[tokio::test]
    async fn update_unssupported_chain_id_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let unsupported_chain_id = 919191919191919191;
        let request = build_request(
            ADDRESS_FOR_MOCK_REQUESTS,
            unsupported_chain_id,
            ADDRESS_FOR_MOCK_REQUESTS,
        );

        let response = update_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(
            format!("chain_id {unsupported_chain_id} is not supported"),
            body.message
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_invalid_replacement_address_ok(#[future] fixture: TestFixture) {
        let fixture = fixture.await;

        let request = build_request(ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, "");

        let response = update_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body: LambdaErrorResponse = serde_json::from_str(response.body()).unwrap();
        assert_eq!("validation", body.code);
        assert_eq!(
            "Invalid H160 value: Invalid input length at line 1 column 22",
            body.message
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_gas_pool_ok(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        let request = build_request(
            ADDRESS_FOR_MOCK_REQUESTS,
            CHAIN_ID_FOR_MOCK_REQUESTS,
            ADDRESS_FOR_MOCK_REQUESTS,
        );

        fixture
            .mock_sponsor_address_config_repository
            .expect_put_address_gas_pool()
            .once()
            .returning(|_, _, _| Ok(()));

        let response = update_gas_pool(
            request,
            &fixture.get_state(),
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::OK, response.status());
    }
}
