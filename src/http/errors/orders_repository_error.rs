use anyhow::anyhow;
use http::Response;

use super::{not_found_response, unknown_error_response};
use crate::result::error::LambdaError;
use repositories::orders::OrdersRepositoryError;

pub const ORDER_NOT_FOUND: &str = "order_not_found";

pub fn orders_repository_error_into_http_response(e: OrdersRepositoryError) -> Response<String> {
    match e {
        OrdersRepositoryError::Unknown(e) => unknown_error_response(LambdaError::Unknown(e)),
        OrdersRepositoryError::OrderNotFound(message) => {
            not_found_response(ORDER_NOT_FOUND, message)
        }
        OrdersRepositoryError::PreviousStatesNotFound => {
            unknown_error_response(LambdaError::Unknown(anyhow!(
                "tried to transition state without checking previous state in code"
            )))
        }
        OrdersRepositoryError::ConditionalCheckFailed(message) => {
            unknown_error_response(LambdaError::Unknown(anyhow!(message)))
        }
    }
}
