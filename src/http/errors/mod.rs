pub mod orders_repository_error;

use crate::result::error::LambdaError;
use lambda_http::Response;
use reqwest::StatusCode;
use serde_json::json;

use crate::result::error::OrchestrationError;

// error codes
pub const NOT_FOUND_ERROR_CODE: &str = "not_found";
pub const SERVER_ERROR_CODE: &str = "server_error";
pub const UNAUTHORIZED_ERROR_CODE: &str = "unauthorized";
pub const VALIDATION_ERROR_CODE: &str = "validation";
pub const UNPROCESSABLE_ERROR_CODE: &str = "unprocessable";
pub const UNSUPPORTED_MEDIA_ERROR_CODE: &str = "unsupported_media_type";

// messages
pub const INCOMPATIBLE_ORDER_REPLACEMENT_ERROR_MESSAGE: &str = "Error setting new gas values";
pub const SERVER_ERROR_MESSAGE: &str = "internal server error";
pub const UNAUTHORIZED_ERROR_MESSAGE: &str = "client is not authorized to make this call";
pub const UNSUPPORTED_MEDIA_ERROR_MESSAGE: &str = "media type specified in header not supported";

impl From<OrchestrationError> for HttpError {
    fn from(error: OrchestrationError) -> Self {
        match error {
            OrchestrationError::Unknown(_) => Self {
                code: SERVER_ERROR_CODE,
                message: SERVER_ERROR_MESSAGE.to_owned(), // do not expose internal error details
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            },
            OrchestrationError::Validation(message) => Self {
                code: VALIDATION_ERROR_CODE,
                message,
                status_code: StatusCode::BAD_REQUEST,
            },
            OrchestrationError::NotFound(message) => Self {
                code: NOT_FOUND_ERROR_CODE,
                message,
                status_code: StatusCode::NOT_FOUND,
            },
        }
    }
}

pub struct HttpError {
    pub code: &'static str,
    pub message: String,
    pub status_code: StatusCode,
}

fn error_response(
    code: &'static str,
    message: String,
    status_code: StatusCode,
    cause: Option<LambdaError>,
) -> Response<String> {
    if let Some(e) = cause {
        tracing::error!(error = ?e, "{:?}", e);
    }
    let mut response = Response::new(error_response_body(code, message));
    let status = response.status_mut();
    *status = status_code;

    response
}

pub fn not_found_response(code: &'static str, message: String) -> Response<String> {
    error_response(code, message, StatusCode::NOT_FOUND, None)
}

pub fn error_response_body(code: &'static str, message: String) -> String {
    json!({
        "code": code,
        "message": message,
    })
    .to_string()
}

pub fn unknown_error_response(cause: LambdaError) -> Response<String> {
    error_response(
        SERVER_ERROR_CODE,
        SERVER_ERROR_MESSAGE.to_owned(),
        StatusCode::INTERNAL_SERVER_ERROR,
        Some(cause),
    )
}

pub fn validation_error_response(message: String, cause: Option<LambdaError>) -> Response<String> {
    error_response(
        VALIDATION_ERROR_CODE,
        message,
        StatusCode::BAD_REQUEST,
        cause,
    )
}

pub fn unauthorized_error_response(cause: Option<LambdaError>) -> Response<String> {
    error_response(
        UNAUTHORIZED_ERROR_CODE,
        UNAUTHORIZED_ERROR_MESSAGE.to_string(),
        StatusCode::UNAUTHORIZED,
        cause,
    )
}

pub fn unsupported_media_error_response(cause: Option<LambdaError>) -> Response<String> {
    error_response(
        UNSUPPORTED_MEDIA_ERROR_CODE,
        UNSUPPORTED_MEDIA_ERROR_MESSAGE.to_string(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        cause,
    )
}

pub fn unprocessable_entity_error_response(message: String) -> Response<String> {
    error_response(
        UNPROCESSABLE_ERROR_CODE,
        message,
        StatusCode::UNPROCESSABLE_ENTITY,
        None,
    )
}
