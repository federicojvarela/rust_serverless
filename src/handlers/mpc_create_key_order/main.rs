use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::step_functions::get_step_functions_client;
use config::Config;
use dtos::requests::KeyRequestBody;
use dtos::responses::{KeyOrderStateMachinePayload, Payload};
use http::StatusCode;
use lambda_http::{run, Error, Request};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::{unknown_error_response, validation_error_response};
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::http_lambda_main;
use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::{
    invoke_step_function_async, StepFunctionConfig,
};
use mpc_signature_sm::lambda_structure::http_lambda_main::{
    CustomFieldsExtractor, HttpLambdaResponse, RequestExtractor,
};
use mpc_signature_sm::model::step_function::StepFunctionContext;
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::validations::http::content_type::validate_content_type;
use rusoto_stepfunctions::StepFunctions;
use serde_json::json;
use tower::service_fn;
use uuid::Uuid;
use validator::Validate;

mod config;
mod dtos;

const EMPTY_CLIENT_USER_ID_MESSAGE: &str = "empty value `client_user_id`";

pub struct State<T: StepFunctions> {
    pub step_functions_client: T,
    pub config: StepFunctionConfig,
}

http_lambda_main!(
    {
        let step_functions_client = get_step_functions_client();
        let config = ConfigLoader::load_default::<Config>();

        State {
            step_functions_client,
            config: config.into(),
        }
    },
    create_key_order,
    [validate_content_type]
);

async fn create_key_order(
    request: Request,
    state: &State<impl StepFunctions>,
    _feature_flags: &FeatureFlags,
) -> HttpLambdaResponse {
    let client_id = request.extract_client_id()?;
    let body: KeyRequestBody = request.extract_body()?;

    body.validate()
        .map_err(|_| validation_error_response(EMPTY_CLIENT_USER_ID_MESSAGE.to_owned(), None))?;

    let client_user_id = body.client_user_id;
    let owning_user_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();

    let step_function_payload = serde_json::to_value(KeyOrderStateMachinePayload {
        payload: Payload {
            client_user_id,
            owning_user_id,
            client_id: client_id.clone(),
        },
        context: StepFunctionContext { order_id },
    })
    .map_err(|e| {
        unknown_error_response(LambdaError::Unknown(anyhow::anyhow!(
            "unable to create state machine json payload: {e:?}"
        )))
    })?;

    invoke_step_function_async(
        client_id,
        step_function_payload,
        &state.step_functions_client,
        &state.config,
        order_id.to_string(),
    )
    .await
    .map_err(unknown_error_response)?;

    LambdaProxyHttpResponse {
        status_code: StatusCode::ACCEPTED,
        body: Some(json!({ "order_id": order_id }).to_string()),
        ..LambdaProxyHttpResponse::default()
    }
    .try_into()
}

#[cfg(test)]
mod tests {
    use http::StatusCode;
    use lambda_http::Body;
    use rstest::{fixture, rstest};
    use rusoto_stepfunctions::StartExecutionOutput;
    use serde_json::json;

    use crate::{create_key_order, State, EMPTY_CLIENT_USER_ID_MESSAGE};
    use common::test_tools::dtos::{Error, OrderAcceptedBody};
    use common::test_tools::http::helpers::build_request_custom_auth;
    use common::test_tools::mocks::step_client::MockStepsClient;
    use mpc_signature_sm::feature_flags::FeatureFlags;
    use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::StepFunctionConfig;

    struct Fixture {
        step_functions_client: MockStepsClient,
        step_functions_config: StepFunctionConfig,
    }

    #[fixture]
    fn test_fixture() -> Fixture {
        Fixture {
            step_functions_client: MockStepsClient::new(),
            step_functions_config: StepFunctionConfig {
                step_function_arn: "some::arn".to_owned(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_response_is_valid_ok(mut test_fixture: Fixture) {
        test_fixture
            .step_functions_client
            .expect_start_execution()
            .times(1)
            .returning(|_| Ok(StartExecutionOutput::default()));

        let body = Body::Text(
            json!({
                "client_user_id": "d906936b-09a6-4f03-8420-93762da8b9a9",
            })
            .to_string(),
        );
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, body);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap();

        assert_eq!(StatusCode::ACCEPTED, response.status());
        serde_json::from_str::<OrderAcceptedBody>(response.body().as_str())
            .expect("Could not deserialized success body");
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_empty_body(test_fixture: Fixture) {
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, Body::Empty);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, "body was empty");
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_invalid_body(test_fixture: Fixture) {
        let body = Body::Text(r#"{ "invalid": json }"#.to_string());
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, body);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(
            body.message,
            "body failed to be converted to a json object".to_owned()
        );
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_missing_client_user_id(test_fixture: Fixture) {
        let body = Body::Text(json!({}).to_string());
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, body);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(
            body.message,
            "missing field `client_user_id` at line 1 column 2"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_empty_client_user_id(test_fixture: Fixture) {
        let body = Body::Text(
            json!({
                "client_user_id": ""
            })
            .to_string(),
        );
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, body);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(body.message, EMPTY_CLIENT_USER_ID_MESSAGE);
    }

    #[rstest]
    #[tokio::test]
    async fn error_bad_request_non_string_client_user_id(test_fixture: Fixture) {
        let body = Body::Text(
            json!({
                "client_user_id": 1
            })
            .to_string(),
        );
        let auth = json!({ "client_id": "some_client_id" });
        let request = build_request_custom_auth(auth, body);

        let response = create_key_order(
            request,
            &State {
                step_functions_client: test_fixture.step_functions_client,
                config: test_fixture.step_functions_config,
            },
            &FeatureFlags::default_in_memory(),
        )
        .await
        .unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, "validation");
        assert_eq!(
            body.message,
            "invalid type: integer `1`, expected a string at line 1 column 19"
        );
    }
}
