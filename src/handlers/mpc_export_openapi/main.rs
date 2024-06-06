mod config;

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use common::aws_clients::{
    api_gateway::get_api_gateway_client, secrets_manager::get_secrets_provider,
};
use config::Config;
use http::StatusCode;
use lambda_http::{run, Error, Request, Response};
use mpc_signature_sm::feature_flags::FeatureFlags;
use mpc_signature_sm::http::errors::unknown_error_response;
use mpc_signature_sm::http::lambda_proxy::LambdaProxyHttpResponse;
use mpc_signature_sm::result::error::LambdaError;
use rusoto_apigateway::{ApiGateway, GetExportRequest};
use serde_json::Value;
use std::collections::HashMap;
use tower::service_fn;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, reload};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let (filter, reload_handle) = reload::Layer::new(LevelFilter::WARN);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::Layer::default().without_time())
        .init();
    let reload_handle = &reload_handle;

    let config = ConfigLoader::load_default::<Config>();

    let apig_client = get_api_gateway_client();

    let service = |_request: Request| async {
        let secrets_provider = get_secrets_provider().await;
        let feature_flags = FeatureFlags::new(secrets_provider).await;
        if feature_flags.get_verbose_mode_flag() {
            reload_handle
                .modify(|filter| *filter = LevelFilter::INFO)
                .unwrap_or_else(|e| tracing::error!(error = ?e, "{:?}", e));
        }

        let response = match export_openapi(&apig_client, &config).await {
            Ok(v) => LambdaProxyHttpResponse {
                status_code: StatusCode::OK,
                headers: HashMap::from([
                    ("Content-Type".to_owned(), "application/json".to_owned()),
                    ("Access-Control-Allow-Origin".to_owned(), "*".to_owned()),
                ]),
                body: Some(v.to_string()),
            }
            .try_into(),
            Err(e) => Ok(unknown_error_response(e)),
        };

        let response: Result<Response<String>, Error> = match response {
            Ok(v) => Ok(v),
            Err(e) => Ok(e),
        };
        response
    };

    run(service_fn(service)).await
}

async fn export_openapi(
    apig_client: &impl ApiGateway,
    config: &Config,
) -> Result<Value, LambdaError> {
    let request = GetExportRequest {
        accepts: Some("application/json".to_owned()),
        export_type: "oas30".to_owned(),
        parameters: Some(HashMap::from([(
            "extensions".to_owned(),
            "apigateway".to_owned(),
        )])),
        rest_api_id: config.api_gateway_rest_api_id.clone(),
        stage_name: config.api_gateway_stage_name.clone(),
    };

    apig_client
        .get_export(request)
        .await
        .map_err(|e| LambdaError::Unknown(anyhow!(e)))?
        .body
        .ok_or_else(|| LambdaError::Unknown(anyhow!("Body was empty")))
        .map(|body| serde_json::from_slice::<Value>(&body))?
        .map_err(|e| LambdaError::Unknown(anyhow!(e)))
}

#[cfg(test)]
mod tests {
    use aws_lambda_events::bytes::Bytes;
    use common::test_tools::mocks::apig_client::MockApiGClient;
    use mockall::predicate;
    use rstest::*;
    use rusoto_apigateway::*;
    use rusoto_core::RusotoError;
    use serde_json::json;

    use super::*;

    const REST_API_ID: &str = "some_id";
    const STAGE_NAME: &str = "some_name";

    struct TestFixture {
        pub client: MockApiGClient,
        pub config: Config,
        pub request: GetExportRequest,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        let client = MockApiGClient::new();
        let config = Config {
            api_gateway_rest_api_id: REST_API_ID.to_owned(),
            api_gateway_stage_name: STAGE_NAME.to_owned(),
        };
        let request = GetExportRequest {
            accepts: Some("application/json".to_owned()),
            export_type: "oas30".to_owned(),
            parameters: Some(HashMap::from([(
                "extensions".to_owned(),
                "apigateway".to_owned(),
            )])),
            rest_api_id: config.api_gateway_rest_api_id.clone(),
            stage_name: config.api_gateway_stage_name.clone(),
        };
        TestFixture {
            client,
            config,
            request,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_export_fails(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .client
            .expect_get_export()
            .with(predicate::eq(fixture.request))
            .times(1)
            .returning(move |_| {
                Err(RusotoError::Service(GetExportError::BadRequest(
                    "".to_owned(),
                )))
            });
        export_openapi(&fixture.client, &fixture.config)
            .await
            .unwrap_err();
    }

    #[rstest]
    #[tokio::test]
    async fn handle_export_returns_empty(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .client
            .expect_get_export()
            .with(predicate::eq(fixture.request))
            .times(1)
            .returning(move |_| Ok(ExportResponse::default()));
        export_openapi(&fixture.client, &fixture.config)
            .await
            .unwrap_err();
    }

    #[rstest]
    #[tokio::test]
    async fn handle_export_returns_non_json(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .client
            .expect_get_export()
            .with(predicate::eq(fixture.request))
            .times(1)
            .returning(move |_| {
                Ok(ExportResponse {
                    body: Some(Bytes::from_static("string".as_bytes())),
                    ..ExportResponse::default()
                })
            });
        export_openapi(&fixture.client, &fixture.config)
            .await
            .unwrap_err();
    }

    #[rstest]
    #[tokio::test]
    async fn handle_export_succeeds(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .client
            .expect_get_export()
            .with(predicate::eq(fixture.request))
            .times(1)
            .returning(move |_| {
                Ok(ExportResponse {
                    body: Some(Bytes::copy_from_slice(
                        json!({"key": "value"}).to_string().as_bytes(),
                    )),
                    ..ExportResponse::default()
                })
            });
        let response = export_openapi(&fixture.client, &fixture.config)
            .await
            .unwrap();
        assert_eq!(json!({"key": "value"}).to_string(), response.to_string());
    }
}
