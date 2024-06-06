use std::str::FromStr;

use anyhow::anyhow;
use http::header::ToStrError;
use http::Response;
use lambda_http::aws_lambda_events::apigw::ApiGatewayProxyRequestContext;
use lambda_http::request::RequestContext;
use lambda_http::{Body, Request, RequestExt};
use serde::de::DeserializeOwned;

use crate::http::errors::{
    unauthorized_error_response, unknown_error_response, validation_error_response,
};
use crate::result::error::LambdaError;

/// Header name to extract the client id
const CLIENT_ID_HEADER_NAME: &str = "client_id";

pub type HttpLambdaResponse = Result<Response<String>, Response<String>>;

// This macro is intended for lambdas that directly interact with the ApiGateway (internally named
// 'http lambdas'). It is used to reduce boilerplate, to preserve state between executions and to
// take advantage of the `?` operator.
//
// Now, when using this macro, an error can be returned as a HTTP response using the `?` operator.
// This allow us to return errors in a more "rusty" way and reduce lines of codes that handle the
// error cases
//
// This macro support request validation as a third parameter. It validates the request it before
// the business logic is executed. The general idea is to declare the validation functions with
// the signature `Fn(&Request) -> Result<(), Response<String>>` and place it in the
// `<root>/src/validations/http/` submodule.
//
// Example usage:
// ```
// http_lambdamain!(
// { .. Sate },
// main_fn,
// [
//   validation_1,
//  validation_2,
//  ..
//  validation_n
// ]
// )
#[macro_export]
macro_rules! http_lambda_main {
    ($persisted_block:block, $handler: ident) => {
        http_lambda_main!($persisted_block, $handler, []);
    };
    ($persisted_block:block, $handler: ident, [$($validation:ident),*]) => {
        #[tokio::main]
        async fn main() -> Result<(), Error> {
            use anyhow::anyhow;
            use common::aws_clients::secrets_manager::get_secrets_provider;
            use http::{HeaderValue, Response};
            use lambda_http::request::RequestContext;
            use lambda_http::{Body, RequestExt};
            use tracing_subscriber::{filter::LevelFilter, prelude::*, reload};
            use mpc_signature_sm::feature_flags::FeatureFlags;
            use mpc_signature_sm::http::errors::unauthorized_error_response;
            use mpc_signature_sm::lambda_structure::http_lambda_main::{RequestExtractor};
            use tracing_log::LogTracer;
            use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};

            LogTracer::init()?;

            let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
            let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
            let bunyan_formatting_layer =
                BunyanFormattingLayer::new(app_name.to_string(), non_blocking_writer);

            // Instantiate a tracing subscriber with reloadable level filter
            let (filter, reload_handle) = reload::Layer::new(LevelFilter::WARN);
            tracing_subscriber::registry()
                .with(filter)
                .with(JsonStorageLayer)
                .with(bunyan_formatting_layer)
                .init();

            let persisted = { $persisted_block };

            let service = |mut request: Request| async {
                let secrets_provider = get_secrets_provider().await;
                let feature_flags = FeatureFlags::new(secrets_provider).await;
                let verbose_mode = feature_flags.get_verbose_mode_flag();
                if verbose_mode {
                    reload_handle
                        .modify(|filter| *filter = LevelFilter::INFO)
                        .unwrap_or_else(|e| tracing::error!(error= ?e, "{:?}", e));
                } else {
                    reload_handle
                        .modify(|filter| *filter = LevelFilter::WARN)
                        .unwrap_or_else(|e| tracing::error!(error= ?e, "{:?}", e));
                }

                let payload = match request.body() {
                    Body::Empty => "No Payload".to_owned(),
                    _ => match request.extract_body::<serde_json::Value>() {
                        Ok(payload) => payload.to_string(),
                        Err(e) => return Ok(e.into()),
                    }
                };
                let context = match request.extract_context() {
                    Ok(context) => context,
                    Err(e) => return Ok(e.into()),
                };
                tracing::info!(payload = ?payload, context = ?context, "Execution started");

                $(
                if let Err(response) = $validation(&request) {
                    return Ok(response);
                }
                )*

                let response: Result<Response<String>, Error> =
                    match $handler(request, &persisted, &feature_flags).await {
                        Ok(response) => Ok(response),
                        Err(response) => Ok(response),
                    };

                response
            };

            run(service_fn(service)).await
        }
    };
}

pub trait RequestExtractor {
    fn extract_path_param<T: DeserializeOwned + FromStr>(
        &self,
        param_name: &str,
    ) -> Result<T, RequestExtractorError>;

    fn extract_header<T: DeserializeOwned + FromStr>(
        &self,
        header_name: &str,
    ) -> Result<T, RequestExtractorError>;

    fn extract_body<T: DeserializeOwned>(&self) -> Result<T, RequestExtractorError>;

    fn extract_context(&self) -> Result<ApiGatewayProxyRequestContext, RequestExtractorError>;
}

impl RequestExtractor for Request {
    fn extract_path_param<T: DeserializeOwned + FromStr>(
        &self,
        param_name: &str,
    ) -> Result<T, RequestExtractorError> {
        let path_parameter = self.path_parameters();
        match path_parameter.first(param_name) {
            None => Err(RequestExtractorError::PathParamNotFoundError(
                param_name.to_owned(),
            )),
            Some(value) => T::from_str(value).map_err(|_| {
                RequestExtractorError::PathParamWithWrongTypeError(param_name.to_owned())
            }),
        }
    }

    fn extract_header<T: DeserializeOwned + FromStr>(
        &self,
        header_name: &str,
    ) -> Result<T, RequestExtractorError> {
        let headers = self.headers();
        match headers.get(header_name) {
            None => Err(RequestExtractorError::HeaderNotFoundError(
                header_name.to_string(),
            )),
            Some(value) => {
                let val = value
                    .to_str()
                    .map_err(RequestExtractorError::HeaderDeserializingError)?;
                T::from_str(val).map_err(|_| {
                    RequestExtractorError::HeaderWithWrongTypeError(header_name.to_string())
                })
            }
        }
    }

    fn extract_body<T: DeserializeOwned>(&self) -> Result<T, RequestExtractorError> {
        match self.body() {
            Body::Text(json_str) => Ok(serde_json::from_str(json_str)
                .map_err(RequestExtractorError::BodyDeserializationError)?),
            Body::Empty => Err(RequestExtractorError::BodyIsEmptyError),
            _ => Err(RequestExtractorError::BodyWithWrongTypeError),
        }
    }

    fn extract_context(&self) -> Result<ApiGatewayProxyRequestContext, RequestExtractorError> {
        match self.request_context() {
            RequestContext::ApiGatewayV1(context) => Ok(context),
            _ => Err(RequestExtractorError::RequestContextNotFoundError),
        }
    }
}

pub trait CustomFieldsExtractor {
    fn extract_client_id(&self) -> Result<String, RequestExtractorError>;
}

impl CustomFieldsExtractor for Request {
    fn extract_client_id(&self) -> Result<String, RequestExtractorError> {
        match self.request_context() {
            RequestContext::ApiGatewayV1(context) => {
                let client_id = context
                    .authorizer
                    .get("claims")
                    .and_then(|claims| claims.get(CLIENT_ID_HEADER_NAME))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| RequestExtractorError::AuthorizerNotFoundError)?;
                Ok(client_id)
            }
            _ => Err(RequestExtractorError::RequestContextNotFoundError),
        }
    }
}

pub enum RequestExtractorError {
    PathParamNotFoundError(String),
    PathParamWithWrongTypeError(String),
    HeaderNotFoundError(String),
    HeaderWithWrongTypeError(String),
    HeaderDeserializingError(ToStrError),
    BodyIsEmptyError,
    BodyWithWrongTypeError,
    BodyDeserializationError(serde_json::Error),
    RequestContextNotFoundError,
    AuthorizerNotFoundError,
}

impl From<RequestExtractorError> for Response<String> {
    fn from(error: RequestExtractorError) -> Self {
        match error {
            RequestExtractorError::PathParamNotFoundError(param_name) => {
                validation_error_response(format!("{param_name} not found in request path"), None)
            }
            RequestExtractorError::PathParamWithWrongTypeError(param_name) => {
                validation_error_response(
                    format!("{param_name} with wrong type in request path"),
                    None,
                )
            }
            RequestExtractorError::HeaderNotFoundError(header_name) => validation_error_response(
                format!("{header_name} not found in request headers"),
                None,
            ),
            RequestExtractorError::HeaderWithWrongTypeError(header_name) => {
                validation_error_response(
                    format!("{header_name} with wrong type in request headers"),
                    None,
                )
            }
            RequestExtractorError::HeaderDeserializingError(e) => {
                unknown_error_response(LambdaError::Unknown(anyhow!(e)))
            }
            RequestExtractorError::BodyIsEmptyError => {
                validation_error_response("body was empty".to_owned(), None)
            }
            RequestExtractorError::BodyWithWrongTypeError => {
                validation_error_response("body wasn't a text type".to_owned(), None)
            }
            RequestExtractorError::BodyDeserializationError(e) => {
                let message =
                    if e.is_data() && !e.to_string().contains("data did not match any variant") {
                        e.to_string()
                    } else {
                        "body failed to be converted to a json object".to_owned()
                    };
                validation_error_response(message, None)
            }
            RequestExtractorError::RequestContextNotFoundError => unauthorized_error_response(
                Some(LambdaError::Unknown(anyhow!("RequestContext not found"))),
            ),
            RequestExtractorError::AuthorizerNotFoundError => unauthorized_error_response(Some(
                LambdaError::Unknown(anyhow!("authorizer not found")),
            )),
        }
    }
}
